use super::*;

const HISTORY_LIMIT: usize = 200;

#[derive(Clone, Debug)]
pub struct DisplayChange {
    pub object_id: String,
    pub before: DisplayProperties,
    pub after: DisplayProperties,
}

#[derive(Clone, Debug)]
pub(crate) enum RemovedObject {
    Layer { index: usize, layer: LayerView },
    Baseplate { index: usize, baseplate: Baseplate },
}

#[derive(Clone, Debug)]
pub enum DocumentCommand {
    SetDisplay(Vec<DisplayChange>),
    AddBaseplate { index: usize, baseplate: Baseplate },
    RemoveObject(RemovedObject),
}

impl DocumentCommand {
    pub fn add_baseplate(
        document: &mut ProjectDocument,
        name: impl Into<String>,
        bounds: Bounds2d,
    ) -> Self {
        let id = BaseplateId(document.id_allocator.allocate());
        Self::AddBaseplate {
            index: document.baseplates.len(),
            baseplate: Baseplate {
                id,
                display: DisplayProperties::baseplate(name),
                default_bounds: Some(bounds.clone()),
                bounds,
            },
        }
    }

    pub fn remove_object(document: &ProjectDocument, object_id: &str) -> anyhow::Result<Self> {
        if let Some(id) = object_id.strip_prefix("layer-") {
            let id = LayerViewId(id.parse()?);
            let (index, _, layer) = document
                .layer_views
                .get_full(&id)
                .ok_or_else(|| anyhow::anyhow!("scene object not found"))?;
            return Ok(Self::RemoveObject(RemovedObject::Layer {
                index,
                layer: layer.clone(),
            }));
        }
        if let Some(id) = object_id.strip_prefix("baseplate-") {
            let id = BaseplateId(id.parse()?);
            let (index, _, baseplate) = document
                .baseplates
                .get_full(&id)
                .ok_or_else(|| anyhow::anyhow!("scene object not found"))?;
            return Ok(Self::RemoveObject(RemovedObject::Baseplate {
                index,
                baseplate: baseplate.clone(),
            }));
        }
        anyhow::bail!("scene object not found")
    }

    fn apply(&self, document: &mut ProjectDocument) -> anyhow::Result<()> {
        match self {
            Self::SetDisplay(changes) => set_displays(document, changes, false)?,
            Self::AddBaseplate { index, baseplate } => {
                if document.baseplates.contains_key(&baseplate.id) {
                    anyhow::bail!("baseplate already exists");
                }
                document.baseplates.shift_insert(
                    (*index).min(document.baseplates.len()),
                    baseplate.id,
                    baseplate.clone(),
                );
            }
            Self::RemoveObject(RemovedObject::Layer { layer, .. }) => {
                if document.layer_views.shift_remove(&layer.id).is_none() {
                    anyhow::bail!("layer no longer exists");
                }
            }
            Self::RemoveObject(RemovedObject::Baseplate { baseplate, .. }) => {
                if document.baseplates.shift_remove(&baseplate.id).is_none() {
                    anyhow::bail!("baseplate no longer exists");
                }
            }
        }
        document.touch();
        Ok(())
    }

    fn undo(&self, document: &mut ProjectDocument) -> anyhow::Result<()> {
        match self {
            Self::SetDisplay(changes) => set_displays(document, changes, true)?,
            Self::AddBaseplate { baseplate, .. } => {
                if document.baseplates.shift_remove(&baseplate.id).is_none() {
                    anyhow::bail!("baseplate no longer exists");
                }
            }
            Self::RemoveObject(RemovedObject::Layer { index, layer }) => {
                if document.layer_views.contains_key(&layer.id) {
                    anyhow::bail!("layer already exists");
                }
                document.layer_views.shift_insert(
                    (*index).min(document.layer_views.len()),
                    layer.id,
                    layer.clone(),
                );
            }
            Self::RemoveObject(RemovedObject::Baseplate { index, baseplate }) => {
                if document.baseplates.contains_key(&baseplate.id) {
                    anyhow::bail!("baseplate already exists");
                }
                document.baseplates.shift_insert(
                    (*index).min(document.baseplates.len()),
                    baseplate.id,
                    baseplate.clone(),
                );
            }
        }
        document.touch();
        Ok(())
    }
}

fn set_displays(
    document: &mut ProjectDocument,
    changes: &[DisplayChange],
    restore_before: bool,
) -> anyhow::Result<()> {
    for change in changes {
        let display = document
            .display_mut(&change.object_id)
            .ok_or_else(|| anyhow::anyhow!("scene object not found: {}", change.object_id))?;
        *display = if restore_before {
            change.before.clone()
        } else {
            change.after.clone()
        };
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct CommandHistory {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    current_state: u64,
    saved_state: u64,
    next_state: u64,
}

#[derive(Debug)]
struct HistoryEntry {
    command: DocumentCommand,
    before_state: u64,
    after_state: u64,
}

impl CommandHistory {
    pub fn execute(
        &mut self,
        document: &mut ProjectDocument,
        command: DocumentCommand,
    ) -> anyhow::Result<()> {
        command.apply(document)?;
        self.next_state = self.next_state.wrapping_add(1);
        let entry = HistoryEntry {
            command,
            before_state: self.current_state,
            after_state: self.next_state,
        };
        self.current_state = entry.after_state;
        self.undo.push(entry);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self, document: &mut ProjectDocument) -> anyhow::Result<bool> {
        let Some(entry) = self.undo.pop() else {
            return Ok(false);
        };
        if let Err(error) = entry.command.undo(document) {
            self.undo.push(entry);
            return Err(error);
        }
        self.current_state = entry.before_state;
        self.redo.push(entry);
        Ok(true)
    }

    pub fn redo(&mut self, document: &mut ProjectDocument) -> anyhow::Result<bool> {
        let Some(entry) = self.redo.pop() else {
            return Ok(false);
        };
        if let Err(error) = entry.command.apply(document) {
            self.redo.push(entry);
            return Err(error);
        }
        self.current_state = entry.after_state;
        self.undo.push(entry);
        Ok(true)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.current_state = 0;
        self.saved_state = 0;
        self.next_state = 0;
    }

    pub fn mark_saved(&mut self) {
        self.saved_state = self.current_state;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.current_state != self.saved_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Bounds2d {
        Bounds2d {
            min_x: -1.0,
            min_y: -2.0,
            max_x: 3.0,
            max_y: 4.0,
        }
    }

    #[test]
    fn restores_baseplate_commands() {
        let mut document = ProjectDocument::default();
        let mut history = CommandHistory::default();
        let command = DocumentCommand::add_baseplate(&mut document, "Baseplate 1", bounds());
        history
            .execute(&mut document, command)
            .expect("add baseplate");
        assert_eq!(document.baseplates.len(), 1);
        assert!(history.can_undo());
        assert!(history.is_dirty());
        history.mark_saved();
        assert!(!history.is_dirty());

        assert!(history.undo(&mut document).expect("undo baseplate"));
        assert!(document.baseplates.is_empty());
        assert!(history.can_redo());
        assert!(history.is_dirty());
        assert!(history.redo(&mut document).expect("redo baseplate"));
        assert_eq!(document.baseplates.len(), 1);
        assert!(!history.is_dirty());

        let object_id = format!("baseplate-{}", document.baseplates[0].id.0);
        let command =
            DocumentCommand::remove_object(&document, &object_id).expect("remove command");
        history
            .execute(&mut document, command)
            .expect("remove baseplate");
        assert!(document.baseplates.is_empty());
        assert!(history.undo(&mut document).expect("restore baseplate"));
        assert_eq!(document.baseplates.len(), 1);
    }

    #[test]
    fn restores_display_changes() {
        let mut document = ProjectDocument::default();
        let id = document.add_baseplate("Baseplate 1", bounds());
        let object_id = format!("baseplate-{}", id.0);
        let before = document
            .display_mut(&object_id)
            .cloned()
            .expect("baseplate display");
        let mut after = before.clone();
        after.name = "Substrate".to_owned();
        after.opacity = 0.4;

        let mut history = CommandHistory::default();
        history
            .execute(
                &mut document,
                DocumentCommand::SetDisplay(vec![DisplayChange {
                    object_id: object_id.clone(),
                    before: before.clone(),
                    after: after.clone(),
                }]),
            )
            .expect("change display");
        assert_eq!(
            document.display_mut(&object_id).cloned(),
            Some(after.clone())
        );

        assert!(history.undo(&mut document).expect("undo display"));
        assert_eq!(document.display_mut(&object_id).cloned(), Some(before));
        assert!(history.redo(&mut document).expect("redo display"));
        assert_eq!(document.display_mut(&object_id).cloned(), Some(after));
    }

    #[test]
    fn tracks_saved_branch() {
        let mut document = ProjectDocument::default();
        let mut history = CommandHistory::default();
        let first = DocumentCommand::add_baseplate(&mut document, "Baseplate 1", bounds());
        history.execute(&mut document, first).expect("first change");
        history.mark_saved();

        let second = DocumentCommand::add_baseplate(&mut document, "Baseplate 2", bounds());
        history
            .execute(&mut document, second)
            .expect("second change");
        assert!(history.undo(&mut document).expect("undo second change"));
        assert!(!history.is_dirty());

        let branch = DocumentCommand::add_baseplate(&mut document, "Substrate", bounds());
        history
            .execute(&mut document, branch)
            .expect("branch change");
        assert!(history.is_dirty());
        assert!(!history.can_redo());
    }
}

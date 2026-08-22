use super::gds::{ParsedGdsLayers, parse_gds_layers};
use super::*;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use indexmap::IndexMap;

use crate::archive::source_key_for_path;

#[derive(Clone, Debug)]
pub struct RenderLayer {
    pub object: GdsLayerObject,
    pub occurrences: Vec<Occurrence>,
}

#[derive(Clone, Debug, Default)]
pub struct RenderScene {
    pub layers: Vec<RenderLayer>,
    pub baseplates: Vec<BaseplateObject>,
}

struct LayerCompileState {
    root_cell: CellId,
    layer: i32,
    datatype: i32,
    instance_path: Vec<InstanceId>,
    stack: Vec<CellId>,
    polygons: Vec<Polygon2d>,
    occurrences: Vec<Occurrence>,
}

impl RenderScene {
    pub fn objects(&self) -> Vec<SceneObject> {
        self.layers
            .iter()
            .map(|layer| SceneObject::GdsLayer(layer.object.clone()))
            .chain(self.baseplates.iter().cloned().map(SceneObject::Baseplate))
            .collect()
    }
}

impl ProjectDocument {
    pub fn display_defaults(&self) -> HashMap<String, DisplayDefaults> {
        self.layer_views
            .values()
            .map(|layer| {
                (
                    format!("layer-{}", layer.id.0),
                    layer.display.current_defaults(),
                )
            })
            .chain(self.baseplates.values().map(|baseplate| {
                (
                    format!("baseplate-{}", baseplate.id.0),
                    baseplate.display.current_defaults(),
                )
            }))
            .collect()
    }

    pub fn from_gds(path: &Path, selections: &[GdsLayerSelection]) -> anyhow::Result<Self> {
        let mut document = Self::default();
        document.import_gds(path, selections)?;
        Ok(document)
    }

    pub fn import_gds(
        &mut self,
        path: &Path,
        selections: &[GdsLayerSelection],
    ) -> anyhow::Result<()> {
        if selections.is_empty() {
            anyhow::bail!("select at least one GDS layer");
        }
        let parsed = parse_gds_layers(path)?;
        self.import_parsed_gds(path, parsed, selections)
    }

    fn import_parsed_gds(
        &mut self,
        path: &Path,
        parsed: ParsedGdsLayers,
        selections: &[GdsLayerSelection],
    ) -> anyhow::Result<()> {
        let source_id = SourceId(self.id_allocator.allocate());
        self.sources.insert(
            source_id,
            SourceDocument {
                id: source_id,
                file_path: path.to_path_buf(),
                source_key: source_key_for_path(path),
                embedded_data: None,
            },
        );

        let mut cell_ids = IndexMap::new();
        for name in parsed.cells.keys().filter(|name| !is_metadata_cell(name)) {
            let cell_id = CellId(self.id_allocator.allocate());
            cell_ids.insert(name.clone(), cell_id);
        }
        if cell_ids.is_empty() {
            anyhow::bail!("no GDS cells available for import");
        }

        for (name, parsed_cell) in &parsed.cells {
            let Some(&cell_id) = cell_ids.get(name) else {
                continue;
            };
            let mut shapes = IndexMap::new();
            for parsed_shape in &parsed_cell.shapes {
                let shape_id = ShapeId(self.id_allocator.allocate());
                shapes.insert(
                    shape_id,
                    Shape {
                        id: shape_id,
                        parent_cell: cell_id,
                        layer: parsed_shape.layer,
                        datatype: parsed_shape.datatype,
                        geometry: parsed_shape.geometry.clone(),
                    },
                );
                self.source_map.insert(
                    shape_id,
                    SourceMapEntry {
                        source_id,
                        cell_name: name.clone(),
                        element_index: parsed_shape.element_index,
                        element_kind: parsed_shape.element_kind,
                    },
                );
            }
            self.cells.insert(
                cell_id,
                CellDefinition {
                    id: cell_id,
                    source_id,
                    name: name.clone(),
                    shapes,
                    instances: IndexMap::new(),
                },
            );
        }

        for (name, parsed_cell) in &parsed.cells {
            let Some(&parent_cell) = cell_ids.get(name) else {
                continue;
            };
            let instances = parsed_cell
                .references
                .iter()
                .flat_map(|reference| {
                    reference
                        .transforms
                        .iter()
                        .map(move |transform| (&reference.cell_name, *transform))
                })
                .collect::<Vec<_>>();
            let cell = self
                .cells
                .get_mut(&parent_cell)
                .expect("allocated cell definition must exist");
            for (referenced_name, transform) in instances {
                let &cell_id = cell_ids.get(referenced_name).ok_or_else(|| {
                    anyhow::anyhow!("missing referenced GDS cell {referenced_name}")
                })?;
                let instance_id = InstanceId(self.id_allocator.allocate());
                cell.instances.insert(
                    instance_id,
                    CellInstance {
                        id: instance_id,
                        parent_cell,
                        cell_id,
                        transform,
                    },
                );
            }
        }

        self.root_cells.extend(
            parsed
                .display_cells()
                .into_iter()
                .filter_map(|name| cell_ids.get(&name).copied()),
        );

        for selection in selections {
            let Some(&root_cell) = cell_ids.get(&selection.cell_name) else {
                anyhow::bail!("selected GDS cell is unavailable: {}", selection.cell_name);
            };
            if !self.root_cells.contains(&root_cell) {
                self.root_cells.push(root_cell);
            }
            let layer_view_id = LayerViewId(self.id_allocator.allocate());
            self.layer_views.insert(
                layer_view_id,
                LayerView {
                    id: layer_view_id,
                    root_cell,
                    layer: selection.layer,
                    datatype: selection.datatype,
                    display: DisplayProperties::gds_layer(format!(
                        "L{}/{}",
                        selection.layer, selection.datatype
                    )),
                },
            );
        }

        if self.layer_views.is_empty() {
            anyhow::bail!("no selected GDS layers are available for import");
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub fn from_legacy_render_objects(objects: Vec<SceneObject>) -> Self {
        let mut document = Self::default();
        let mut sources = IndexMap::<String, SourceId>::new();
        for object in objects {
            match object {
                SceneObject::GdsLayer(layer) => {
                    let source_id = *sources.entry(layer.source_key.clone()).or_insert_with(|| {
                        let id = SourceId(document.id_allocator.allocate());
                        document.sources.insert(
                            id,
                            SourceDocument {
                                id,
                                file_path: layer.source_path.clone(),
                                source_key: layer.source_key.clone(),
                                embedded_data: None,
                            },
                        );
                        id
                    });
                    let cell_id = CellId(document.id_allocator.allocate());
                    let mut shapes = IndexMap::new();
                    for (element_index, polygon) in layer.polygons.into_iter().enumerate() {
                        let shape_id = ShapeId(document.id_allocator.allocate());
                        shapes.insert(
                            shape_id,
                            Shape {
                                id: shape_id,
                                parent_cell: cell_id,
                                layer: layer.layer,
                                datatype: layer.datatype,
                                geometry: ShapeKind::Boundary(polygon),
                            },
                        );
                        document.source_map.insert(
                            shape_id,
                            SourceMapEntry {
                                source_id,
                                cell_name: layer.cell_name.clone(),
                                element_index: u64::try_from(element_index)
                                    .expect("legacy GDS element index exceeds u64"),
                                element_kind: GdsElementKind::Boundary,
                            },
                        );
                    }
                    document.cells.insert(
                        cell_id,
                        CellDefinition {
                            id: cell_id,
                            source_id,
                            name: layer.cell_name,
                            shapes,
                            instances: IndexMap::new(),
                        },
                    );
                    document.root_cells.push(cell_id);
                    let layer_view_id = LayerViewId(document.id_allocator.allocate());
                    document.layer_views.insert(
                        layer_view_id,
                        LayerView {
                            id: layer_view_id,
                            root_cell: cell_id,
                            layer: layer.layer,
                            datatype: layer.datatype,
                            display: layer.display,
                        },
                    );
                }
                SceneObject::Baseplate(baseplate) => {
                    let id = BaseplateId(document.id_allocator.allocate());
                    document.baseplates.insert(
                        id,
                        Baseplate {
                            id,
                            display: baseplate.display,
                            bounds: baseplate.bounds,
                            default_bounds: baseplate.default_bounds,
                        },
                    );
                }
            }
        }
        document.touch();
        document
    }

    pub fn compile_render_scene(&self) -> anyhow::Result<RenderScene> {
        let mut render_scene = RenderScene::default();
        for layer_view in self.layer_views.values() {
            let root_cell = self.cell(layer_view.root_cell)?;
            let source = self.source(root_cell.source_id)?;
            let mut state = LayerCompileState {
                root_cell: layer_view.root_cell,
                layer: layer_view.layer,
                datatype: layer_view.datatype,
                instance_path: Vec::new(),
                stack: Vec::new(),
                polygons: Vec::new(),
                occurrences: Vec::new(),
            };
            self.compile_layer_into(layer_view.root_cell, Transform2d::identity(), &mut state)?;
            let Some(bounds) = polygons_bounds(&state.polygons) else {
                continue;
            };
            render_scene.layers.push(RenderLayer {
                object: GdsLayerObject {
                    id: format!("layer-{}", layer_view.id.0),
                    display: layer_view.display.clone(),
                    file_path: source.file_path.clone(),
                    source_path: source.file_path.clone(),
                    source_key: source.source_key.clone(),
                    cell_name: root_cell.name.clone(),
                    layer: layer_view.layer,
                    datatype: layer_view.datatype,
                    bounds,
                    polygons: state.polygons,
                },
                occurrences: state.occurrences,
            });
        }
        render_scene
            .baseplates
            .extend(self.baseplates.values().map(|baseplate| BaseplateObject {
                id: format!("baseplate-{}", baseplate.id.0),
                display: baseplate.display.clone(),
                bounds: baseplate.bounds.clone(),
                default_bounds: baseplate.default_bounds.clone(),
            }));
        Ok(render_scene)
    }

    fn compile_layer_into(
        &self,
        cell_id: CellId,
        transform: Transform2d,
        state: &mut LayerCompileState,
    ) -> anyhow::Result<()> {
        const DEPTH_MAX: usize = 512;
        if state.stack.len() >= DEPTH_MAX {
            anyhow::bail!("document instance depth exceeds {DEPTH_MAX}");
        }
        if state.stack.contains(&cell_id) {
            anyhow::bail!("document contains a cyclic cell instance reference");
        }
        let cell = self.cell(cell_id)?;
        state.stack.push(cell_id);
        for shape in cell.shapes.values() {
            if shape.layer != state.layer || shape.datatype != state.datatype {
                continue;
            }
            let occurrence = Occurrence {
                root_cell: state.root_cell,
                leaf_cell: cell_id,
                instance_path: state.instance_path.clone(),
                shape_id: shape.id,
            };
            for polygon in shape_polygons(&shape.geometry) {
                if let Some(polygon) = transform_polygon(&polygon, transform) {
                    state.polygons.push(polygon);
                    state.occurrences.push(occurrence.clone());
                }
            }
        }
        for instance in cell.instances.values() {
            state.instance_path.push(instance.id);
            self.compile_layer_into(instance.cell_id, transform.then(instance.transform), state)?;
            state.instance_path.pop();
        }
        state.stack.pop();
        Ok(())
    }

    fn cell(&self, cell_id: CellId) -> anyhow::Result<&CellDefinition> {
        self.cells
            .get(&cell_id)
            .ok_or_else(|| anyhow::anyhow!("document cell {} is unavailable", cell_id.0))
    }

    fn source(&self, source_id: SourceId) -> anyhow::Result<&SourceDocument> {
        self.sources
            .get(&source_id)
            .ok_or_else(|| anyhow::anyhow!("document source {} is unavailable", source_id.0))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn inspect_occurrence(
        &self,
        occurrence: &Occurrence,
    ) -> anyhow::Result<OccurrenceInspection> {
        let mut current_cell = occurrence.root_cell;
        let mut hierarchy_path = vec![self.cell(current_cell)?.name.clone()];
        for instance_id in &occurrence.instance_path {
            let instance = self
                .cell(current_cell)?
                .instances
                .get(instance_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("document instance {} is unavailable", instance_id.0)
                })?;
            current_cell = instance.cell_id;
            hierarchy_path.push(self.cell(current_cell)?.name.clone());
        }
        if current_cell != occurrence.leaf_cell {
            anyhow::bail!("occurrence instance path does not resolve to its leaf cell");
        }

        let cell = self.cell(occurrence.leaf_cell)?;
        let shape = cell.shapes.get(&occurrence.shape_id).ok_or_else(|| {
            anyhow::anyhow!("document shape {} is unavailable", occurrence.shape_id.0)
        })?;
        if shape.parent_cell != cell.id {
            anyhow::bail!("document shape ownership is inconsistent");
        }
        let shape_type = match &shape.geometry {
            ShapeKind::Boundary(_) => "Boundary",
            ShapeKind::Path(_) => "Path",
            ShapeKind::Rectangle(_) => "Rectangle",
        };
        Ok(OccurrenceInspection {
            cell_name: cell.name.clone(),
            shape_id: shape.id,
            shape_type: shape_type.to_owned(),
            layer: shape.layer,
            datatype: shape.datatype,
            instance_path: occurrence.instance_path.clone(),
            hierarchy_path,
        })
    }

    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn display_mut(&mut self, render_id: &str) -> Option<&mut DisplayProperties> {
        if let Some(layer_id) = render_id.strip_prefix("layer-") {
            let id = layer_id.parse::<u64>().ok()?;
            return self
                .layer_views
                .get_mut(&LayerViewId(id))
                .map(|layer_view| &mut layer_view.display);
        }
        let id = render_id.strip_prefix("baseplate-")?.parse::<u64>().ok()?;
        self.baseplates
            .get_mut(&BaseplateId(id))
            .map(|baseplate| &mut baseplate.display)
    }

    pub fn remove_render_object(&mut self, render_id: &str) -> bool {
        let removed = if let Some(id) = render_id.strip_prefix("layer-") {
            id.parse::<u64>()
                .ok()
                .and_then(|id| self.layer_views.shift_remove(&LayerViewId(id)))
                .is_some()
        } else if let Some(id) = render_id.strip_prefix("baseplate-") {
            id.parse::<u64>()
                .ok()
                .and_then(|id| self.baseplates.shift_remove(&BaseplateId(id)))
                .is_some()
        } else {
            false
        };
        if removed {
            self.touch();
        }
        removed
    }

    pub fn next_baseplate_name(&self) -> String {
        let prefix = "Baseplate ";
        let used_indices = self
            .baseplates
            .values()
            .filter_map(|baseplate| baseplate.display.name.strip_prefix(prefix))
            .filter_map(|suffix| suffix.parse::<usize>().ok())
            .collect::<HashSet<_>>();
        let mut index = 1;
        while used_indices.contains(&index) {
            index += 1;
        }
        format!("{prefix}{index}")
    }

    pub fn add_baseplate(&mut self, name: impl Into<String>, bounds: Bounds2d) -> BaseplateId {
        let id = BaseplateId(self.id_allocator.allocate());
        self.baseplates.insert(
            id,
            Baseplate {
                id,
                display: DisplayProperties::baseplate(name),
                default_bounds: Some(bounds.clone()),
                bounds,
            },
        );
        self.touch();
        id
    }

    pub fn default_baseplate_bounds(&self, selection: &Selection) -> anyhow::Result<Bounds2d> {
        let render_scene = self.compile_render_scene()?;
        let mut bounds = None;
        for layer in render_scene.layers {
            let matches_selection = match selection {
                Selection::Scene => true,
                Selection::Object(render_id) => layer.object.id == *render_id,
                Selection::Cell(key) => {
                    layer.object.file_path == key.file_path
                        && layer.object.cell_name == key.cell_name
                }
            };
            if matches_selection {
                merge_optional_bounds(&mut bounds, &layer.object.bounds);
            }
        }
        Ok(bounds.unwrap_or(Bounds2d {
            min_x: -100.0,
            min_y: -100.0,
            max_x: 100.0,
            max_y: 100.0,
        }))
    }
}

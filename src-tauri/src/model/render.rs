use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::geometry::merge_optional_bounds;
use super::{Bounds2d, DisplayProperties, Polygon2d};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GdsLayerObject {
    pub id: String,
    pub display: DisplayProperties,
    pub file_path: PathBuf,
    pub source_path: PathBuf,
    pub source_key: String,
    pub cell_name: String,
    pub layer: i32,
    pub datatype: i32,
    pub bounds: Bounds2d,
    #[serde(default)]
    pub polygons: Vec<Polygon2d>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaseplateObject {
    pub id: String,
    pub display: DisplayProperties,
    pub bounds: Bounds2d,
    #[serde(default)]
    pub default_bounds: Option<Bounds2d>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum SceneObject {
    GdsLayer(GdsLayerObject),
    Baseplate(BaseplateObject),
}

impl SceneObject {
    pub fn id(&self) -> &str {
        match self {
            SceneObject::GdsLayer(obj) => &obj.id,
            SceneObject::Baseplate(obj) => &obj.id,
        }
    }

    pub fn display(&self) -> &DisplayProperties {
        match self {
            SceneObject::GdsLayer(obj) => &obj.display,
            SceneObject::Baseplate(obj) => &obj.display,
        }
    }

    pub fn display_mut(&mut self) -> &mut DisplayProperties {
        match self {
            SceneObject::GdsLayer(obj) => &mut obj.display,
            SceneObject::Baseplate(obj) => &mut obj.display,
        }
    }

    pub fn bounds(&self) -> &Bounds2d {
        match self {
            SceneObject::GdsLayer(obj) => &obj.bounds,
            SceneObject::Baseplate(obj) => &obj.bounds,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.display().visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.display_mut().visible = visible;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Scene,
    Object(String),
    Cell(CellKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub file_path: PathBuf,
    pub cell_name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scene {
    objects: IndexMap<String, SceneObject>,
    #[serde(skip)]
    revision: u64,
}

impl Scene {
    pub fn add(&mut self, obj: SceneObject) -> anyhow::Result<()> {
        let id = obj.id().to_owned();
        if self.objects.contains_key(&id) {
            anyhow::bail!("duplicate object id: {id}");
        }
        self.objects.insert(id, obj);
        self.touch();
        Ok(())
    }

    pub fn remove(&mut self, object_id: &str) -> Option<SceneObject> {
        let removed = self.objects.shift_remove(object_id);
        if removed.is_some() {
            self.touch();
        }
        removed
    }

    pub fn get(&self, object_id: &str) -> Option<&SceneObject> {
        self.objects.get(object_id)
    }

    pub fn get_mut(&mut self, object_id: &str) -> Option<&mut SceneObject> {
        self.objects.get_mut(object_id)
    }

    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn objects(&self) -> impl Iterator<Item = &SceneObject> {
        self.objects.values()
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn next_baseplate_name(&self) -> String {
        let prefix = "Baseplate ";
        let mut used_indices = std::collections::HashSet::new();
        for obj in self.objects() {
            let SceneObject::Baseplate(baseplate) = obj else {
                continue;
            };
            let Some(suffix) = baseplate.display.name.strip_prefix(prefix) else {
                continue;
            };
            let Ok(index) = suffix.parse::<usize>() else {
                continue;
            };
            used_indices.insert(index);
        }

        let mut index = 1;
        while used_indices.contains(&index) {
            index += 1;
        }
        format!("{prefix}{index}")
    }

    pub fn cell_groups(&self) -> Vec<CellGroup> {
        let mut groups: IndexMap<CellKey, Vec<String>> = IndexMap::new();
        for obj in self.objects() {
            if let SceneObject::GdsLayer(layer) = obj {
                let key = CellKey {
                    file_path: layer.file_path.clone(),
                    cell_name: layer.cell_name.clone(),
                };
                groups.entry(key).or_default().push(layer.id.clone());
            }
        }

        groups
            .into_iter()
            .map(|(key, object_ids)| CellGroup { key, object_ids })
            .collect()
    }

    pub fn default_baseplate_bounds(&self, selection: &Selection) -> Bounds2d {
        if let Selection::Cell(key) = selection
            && let Some(bounds) = self.bounds_for_cell(key)
        {
            return bounds;
        }

        self.gds_bounds().unwrap_or(Bounds2d {
            min_x: -100.0,
            min_y: -100.0,
            max_x: 100.0,
            max_y: 100.0,
        })
    }

    fn bounds_for_cell(&self, key: &CellKey) -> Option<Bounds2d> {
        let mut bounds = None;
        for obj in self.objects() {
            let SceneObject::GdsLayer(layer) = obj else {
                continue;
            };
            if layer.file_path != key.file_path || layer.cell_name != key.cell_name {
                continue;
            }
            merge_optional_bounds(&mut bounds, &layer.bounds);
        }
        bounds
    }

    fn gds_bounds(&self) -> Option<Bounds2d> {
        let mut bounds = None;
        for obj in self.objects() {
            let SceneObject::GdsLayer(layer) = obj else {
                continue;
            };
            merge_optional_bounds(&mut bounds, &layer.bounds);
        }
        bounds
    }
}

#[derive(Clone, Debug)]
pub struct CellGroup {
    pub key: CellKey,
    pub object_ids: Vec<String>,
}

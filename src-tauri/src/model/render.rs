use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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

    pub fn display_mut(&mut self) -> &mut DisplayProperties {
        match self {
            SceneObject::GdsLayer(obj) => &mut obj.display,
            SceneObject::Baseplate(obj) => &mut obj.display,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Scene,
    Cell(CellKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub file_path: PathBuf,
    pub cell_name: String,
}

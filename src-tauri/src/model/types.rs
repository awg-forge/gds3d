use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::geometry::Transform2d;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds2d {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// A closed 2D polygon from a GDS boundary-like element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Polygon2d {
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub holes: Vec<Vec<[f32; 2]>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShapeId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerViewId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BaseplateId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceDocument {
    pub id: SourceId,
    pub file_path: PathBuf,
    pub source_key: String,
    #[serde(skip)]
    pub embedded_data: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GdsElementKind {
    Boundary,
    Path,
    Box,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapEntry {
    pub source_id: SourceId,
    pub cell_name: String,
    pub element_index: u64,
    pub element_kind: GdsElementKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PathCapStyle {
    #[default]
    Flush,
    Round,
    ExtendedHalfWidth,
    Custom {
        begin_extension: f32,
        end_extension: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathShape {
    pub centerline: Vec<[f32; 2]>,
    pub width: f32,
    #[serde(default)]
    pub cap: PathCapStyle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RectangleShape {
    pub center: [f32; 2],
    pub size: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum ShapeKind {
    Boundary(Polygon2d),
    Path(PathShape),
    Rectangle(RectangleShape),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub id: ShapeId,
    pub parent_cell: CellId,
    pub layer: i32,
    pub datatype: i32,
    pub geometry: ShapeKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellInstance {
    pub id: InstanceId,
    pub parent_cell: CellId,
    pub cell_id: CellId,
    pub transform: Transform2d,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellDefinition {
    pub id: CellId,
    pub source_id: SourceId,
    pub name: String,
    pub shapes: IndexMap<ShapeId, Shape>,
    pub instances: IndexMap<InstanceId, CellInstance>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerView {
    pub id: LayerViewId,
    pub root_cell: CellId,
    pub layer: i32,
    pub datatype: i32,
    pub display: DisplayProperties,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseplate {
    pub id: BaseplateId,
    pub display: DisplayProperties,
    pub bounds: Bounds2d,
    #[serde(default)]
    pub default_bounds: Option<Bounds2d>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub root_cell: CellId,
    pub leaf_cell: CellId,
    pub instance_path: Vec<InstanceId>,
    pub shape_id: ShapeId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAllocator {
    next: u64,
}

impl IdAllocator {
    pub(super) fn allocate(&mut self) -> u64 {
        self.next = self
            .next
            .checked_add(1)
            .expect("document ID space exhausted");
        self.next
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub sources: IndexMap<SourceId, SourceDocument>,
    pub cells: IndexMap<CellId, CellDefinition>,
    pub root_cells: Vec<CellId>,
    pub layer_views: IndexMap<LayerViewId, LayerView>,
    pub baseplates: IndexMap<BaseplateId, Baseplate>,
    pub source_map: IndexMap<ShapeId, SourceMapEntry>,
    #[serde(default)]
    pub(super) id_allocator: IdAllocator,
    #[serde(skip)]
    pub(super) revision: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayProperties {
    pub name: String,
    pub visible: bool,
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    pub z_min: f32,
    pub z_max: f32,
    #[serde(default)]
    pub defaults: DisplayDefaults,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayDefaults {
    pub name: String,
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    pub z_min: f32,
    pub z_max: f32,
}

fn default_opacity() -> f32 {
    1.0
}

impl DisplayProperties {
    pub fn gds_layer(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            defaults: DisplayDefaults {
                name: name.clone(),
                color: "#2D6CDF".to_owned(),
                opacity: 1.0,
                z_min: 0.0,
                z_max: 15.0,
            },
            name,
            visible: true,
            color: "#2D6CDF".to_owned(),
            opacity: 1.0,
            z_min: 0.0,
            z_max: 15.0,
        }
    }

    pub fn baseplate(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            defaults: DisplayDefaults {
                name: name.clone(),
                color: "#5F6B78".to_owned(),
                opacity: 1.0,
                z_min: -15.0,
                z_max: 0.0,
            },
            name,
            visible: true,
            color: "#5F6B78".to_owned(),
            opacity: 1.0,
            z_min: -15.0,
            z_max: 0.0,
        }
    }
}

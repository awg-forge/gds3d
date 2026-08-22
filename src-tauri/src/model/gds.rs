use std::collections::HashSet;
use std::fs;
use std::path::Path;

use gdsii::parser::{Aref, Element, GdsEvent, GdsParser, Sref};
use gdsii::types::GdsPoint;
use indexmap::IndexMap;

use super::geometry::*;
use super::types::{Bounds2d, GdsElementKind, Polygon2d, ShapeKind};

pub(super) fn parse_gds_layers(path: &Path) -> anyhow::Result<ParsedGdsLayers> {
    let data = fs::read(path)?;
    let mut current_cell = None::<String>;
    let mut coordinate_scale = 1.0f32;
    let mut cells = IndexMap::<String, ParsedGdsCell>::new();
    let mut referenced_cells = HashSet::new();

    for event in GdsParser::new(&data) {
        match event? {
            GdsEvent::LibraryBegin(library) => {
                coordinate_scale = parse_coordinate_scale(library.db_in_user)?;
            }
            GdsEvent::StructureBegin(structure) => {
                let cell_name = structure.name.to_owned();
                cells.entry(cell_name.clone()).or_default();
                current_cell = Some(cell_name);
            }
            GdsEvent::StructureEnd => {
                current_cell = None;
            }
            GdsEvent::Element(Element::Boundary(boundary)) => {
                let Some(cell_name) = current_cell.as_ref() else {
                    continue;
                };
                let Some(cell) = cells.get_mut(cell_name) else {
                    continue;
                };
                let element_index = cell.next_element_index();
                add_cell_shape(
                    cell,
                    element_index,
                    boundary.layer,
                    boundary.datatype,
                    polygon_from_points(GdsPoint::iter_xy(boundary.xy.as_ref()), coordinate_scale)
                        .map(ShapeKind::Boundary),
                    GdsElementKind::Boundary,
                );
            }
            GdsEvent::Element(Element::Path(path)) => {
                let Some(cell_name) = current_cell.as_ref() else {
                    continue;
                };
                let Some(cell) = cells.get_mut(cell_name) else {
                    continue;
                };
                let element_index = cell.next_element_index();
                add_cell_shape(
                    cell,
                    element_index,
                    path.layer,
                    path.datatype,
                    path_shape_from_gds(&path, coordinate_scale).map(ShapeKind::Path),
                    GdsElementKind::Path,
                );
            }
            GdsEvent::Element(Element::Box(box_)) => {
                let Some(cell_name) = current_cell.as_ref() else {
                    continue;
                };
                let Some(cell) = cells.get_mut(cell_name) else {
                    continue;
                };
                let element_index = cell.next_element_index();
                add_cell_shape(
                    cell,
                    element_index,
                    box_.layer,
                    box_.boxtype,
                    polygon_from_points(GdsPoint::iter_xy(box_.xy.as_ref()), coordinate_scale)
                        .map(shape_kind_from_box),
                    GdsElementKind::Box,
                );
            }
            GdsEvent::Element(Element::Sref(sref)) => {
                let Some(cell_name) = current_cell.as_ref() else {
                    continue;
                };
                let Some(cell) = cells.get_mut(cell_name) else {
                    continue;
                };
                cell.next_element_index();
                referenced_cells.insert(sref.sname.to_owned());
                if let Some(reference) = CellReference::from_sref(&sref, coordinate_scale) {
                    cell.references.push(reference);
                }
            }
            GdsEvent::Element(Element::Aref(aref)) => {
                let Some(cell_name) = current_cell.as_ref() else {
                    continue;
                };
                let Some(cell) = cells.get_mut(cell_name) else {
                    continue;
                };
                cell.next_element_index();
                referenced_cells.insert(aref.sname.to_owned());
                if let Some(reference) = CellReference::from_aref(&aref, coordinate_scale) {
                    cell.references.push(reference);
                }
            }
            GdsEvent::Element(_) => {
                if let Some(cell_name) = current_cell.as_ref()
                    && let Some(cell) = cells.get_mut(cell_name)
                {
                    cell.next_element_index();
                }
            }
            GdsEvent::Property(_) | GdsEvent::LibraryEnd => {}
        }
    }

    if cells.values().all(ParsedGdsCell::is_empty) {
        anyhow::bail!("no GDS boundary, path, or box geometry found");
    }

    Ok(ParsedGdsLayers {
        cells,
        referenced_cells,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct LayerKey {
    pub(super) cell_name: String,
    pub(super) layer: i32,
    pub(super) datatype: i32,
}

#[derive(Clone, Debug)]
pub(super) struct LayerGeometry {
    pub(super) bounds: Bounds2d,
    pub(super) polygons: Vec<Polygon2d>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ParsedGdsCell {
    pub(super) shapes: Vec<ParsedShape>,
    pub(super) references: Vec<CellReference>,
    element_count: u64,
}

#[derive(Clone, Debug)]
pub(super) struct ParsedShape {
    pub(super) layer: i32,
    pub(super) datatype: i32,
    pub(super) geometry: ShapeKind,
    pub(super) element_kind: GdsElementKind,
    pub(super) element_index: u64,
}

impl ParsedGdsCell {
    fn next_element_index(&mut self) -> u64 {
        let element_index = self.element_count;
        self.element_count = self
            .element_count
            .checked_add(1)
            .expect("GDS cell element count exceeds u64");
        element_index
    }

    fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.references.is_empty()
    }
}

pub(super) struct ParsedGdsLayers {
    pub(super) cells: IndexMap<String, ParsedGdsCell>,
    referenced_cells: HashSet<String>,
}

impl ParsedGdsLayers {
    pub(super) fn display_cells(&self) -> Vec<String> {
        let mut cells = self
            .cells
            .keys()
            .filter(|name| !self.referenced_cells.contains(*name))
            .filter(|name| !is_metadata_cell(name))
            .cloned()
            .collect::<Vec<_>>();
        if cells.is_empty() {
            cells = self
                .cells
                .keys()
                .filter(|name| !is_metadata_cell(name))
                .cloned()
                .collect();
        }
        cells.sort_by_key(|name| name.to_lowercase());
        cells
    }

    pub(super) fn flatten_display_layers(
        &self,
    ) -> anyhow::Result<IndexMap<LayerKey, LayerGeometry>> {
        let mut layers = IndexMap::new();
        for cell_name in self.display_cells() {
            layers.extend(self.flatten_cell_layers(&cell_name)?);
        }
        Ok(layers)
    }

    pub(super) fn flatten_cell_layers(
        &self,
        cell_name: &str,
    ) -> anyhow::Result<IndexMap<LayerKey, LayerGeometry>> {
        let mut layers = IndexMap::new();
        let mut stack = Vec::new();
        self.flatten_cell_into(
            cell_name,
            cell_name,
            Transform2d::identity(),
            &mut stack,
            &mut layers,
        )?;
        for geometry in layers.values_mut() {
            geometry.polygons = union_layer_polygons(std::mem::take(&mut geometry.polygons));
            if let Some(bounds) = polygons_bounds(&geometry.polygons) {
                geometry.bounds = bounds;
            }
        }
        Ok(layers)
    }

    fn flatten_cell_into(
        &self,
        cell_name: &str,
        output_cell_name: &str,
        transform: Transform2d,
        stack: &mut Vec<String>,
        layers: &mut IndexMap<LayerKey, LayerGeometry>,
    ) -> anyhow::Result<()> {
        const DEPTH_MAX: usize = 512;

        if stack.len() >= DEPTH_MAX {
            anyhow::bail!("GDS reference depth exceeds {DEPTH_MAX}");
        }
        if stack.iter().any(|name| name == cell_name) {
            anyhow::bail!("cyclic GDS reference involving cell {cell_name}");
        }
        let Some(cell) = self.cells.get(cell_name) else {
            anyhow::bail!("missing referenced GDS cell {cell_name}");
        };

        stack.push(cell_name.to_owned());
        for shape in &cell.shapes {
            for polygon in shape_polygons(&shape.geometry) {
                add_layer_polygon(
                    layers,
                    output_cell_name,
                    shape.layer,
                    shape.datatype,
                    transform_polygon(&polygon, transform),
                );
            }
        }
        for reference in &cell.references {
            for reference_transform in &reference.transforms {
                self.flatten_cell_into(
                    &reference.cell_name,
                    output_cell_name,
                    transform.then(*reference_transform),
                    stack,
                    layers,
                )?;
            }
        }
        stack.pop();
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) struct CellReference {
    pub(super) cell_name: String,
    pub(super) transforms: Vec<Transform2d>,
}

impl CellReference {
    fn from_sref(sref: &Sref<'_>, coordinate_scale: f32) -> Option<Self> {
        let origin = point_from_xy(sref.xy.as_ref(), 0, coordinate_scale)?;
        Some(Self {
            cell_name: sref.sname.to_owned(),
            transforms: vec![Transform2d::from_strans(sref.strans).with_translation(origin)],
        })
    }

    fn from_aref(aref: &Aref<'_>, coordinate_scale: f32) -> Option<Self> {
        let columns = usize::try_from(aref.colrow.0).ok()?;
        let rows = usize::try_from(aref.colrow.1).ok()?;
        if columns == 0 || rows == 0 {
            return None;
        }

        let origin = point_from_xy(aref.xy.as_ref(), 0, coordinate_scale)?;
        let column_end = point_from_xy(aref.xy.as_ref(), 1, coordinate_scale)?;
        let row_end = point_from_xy(aref.xy.as_ref(), 2, coordinate_scale)?;
        let column_step = step_vector(origin, column_end, columns);
        let row_step = step_vector(origin, row_end, rows);
        let base = Transform2d::from_strans(aref.strans).with_translation(origin);
        let mut transforms = Vec::with_capacity(columns.saturating_mul(rows));

        for row in 0..rows {
            for column in 0..columns {
                transforms.push(base.with_offset([
                    column_step[0] * column as f32 + row_step[0] * row as f32,
                    column_step[1] * column as f32 + row_step[1] * row as f32,
                ]));
            }
        }

        Some(Self {
            cell_name: aref.sname.to_owned(),
            transforms,
        })
    }
}

fn add_cell_shape(
    cell: &mut ParsedGdsCell,
    element_index: u64,
    layer: i16,
    datatype: i16,
    geometry: Option<ShapeKind>,
    element_kind: GdsElementKind,
) {
    let Some(geometry) = geometry else {
        return;
    };
    cell.shapes.push(ParsedShape {
        layer: i32::from(layer),
        datatype: i32::from(datatype),
        geometry,
        element_kind,
        element_index,
    });
}

fn add_layer_polygon(
    layers: &mut IndexMap<LayerKey, LayerGeometry>,
    cell_name: &str,
    layer: i32,
    datatype: i32,
    polygon: Option<Polygon2d>,
) {
    if is_metadata_cell(cell_name) {
        return;
    }

    let Some(polygon) = polygon else {
        return;
    };
    let Some(bounds) = polygon_bounds(&polygon) else {
        return;
    };

    let key = LayerKey {
        cell_name: cell_name.to_owned(),
        layer,
        datatype,
    };
    let layer = layers.entry(key).or_insert_with(|| LayerGeometry {
        bounds: bounds.clone(),
        polygons: Vec::new(),
    });
    merge_bounds(&mut layer.bounds, &bounds);
    layer.polygons.push(polygon);
}

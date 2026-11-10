use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::gds::parse_gds_layers;
use super::types::Bounds2d;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GdsFileInfo {
    pub file_path: PathBuf,
    pub cells: Vec<GdsCellInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GdsCellInfo {
    pub name: String,
    pub layers: Vec<GdsLayerInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GdsLayerInfo {
    pub selection: GdsLayerSelection,
    pub polygon_count: usize,
    pub bounds: Bounds2d,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GdsLayerSelection {
    pub cell_name: String,
    pub layer: i32,
    pub datatype: i32,
}

pub fn inspect_gds_file(path: &Path) -> anyhow::Result<GdsFileInfo> {
    let parsed = parse_gds_layers(path)?;
    let display_cells = parsed.display_cells();
    let mut cells = Vec::new();

    for cell_name in display_cells {
        let mut layers = parsed
            .flatten_cell_layers(&cell_name)?
            .iter()
            .map(|(key, geometry)| GdsLayerInfo {
                selection: GdsLayerSelection {
                    cell_name: key.cell_name.clone(),
                    layer: key.layer,
                    datatype: key.datatype,
                },
                polygon_count: geometry.polygons.len(),
                bounds: geometry.bounds.clone(),
            })
            .collect::<Vec<_>>();
        layers.sort_by_key(|layer| (layer.selection.layer, layer.selection.datatype));
        if !layers.is_empty() {
            cells.push(GdsCellInfo {
                name: cell_name,
                layers,
            });
        }
    }

    if cells.is_empty() {
        anyhow::bail!("no renderable GDS layers found");
    }

    Ok(GdsFileInfo {
        file_path: path.to_path_buf(),
        cells,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::inspect_gds_file;
    use crate::model::{Polygon2d, ProjectDocument, union_layer_polygons};

    #[test]
    fn document_import_preserves_shape_ownership_and_occurrences() {
        let path = Path::new("../models/AWG_0.8nmCS_16CH_0nmOS.gds");
        let info = inspect_gds_file(path).expect("inspect sample GDS");
        let selection = info
            .cells
            .iter()
            .find(|cell| cell.name == "AWG")
            .and_then(|cell| cell.layers.first())
            .map(|layer| layer.selection.clone())
            .expect("AWG layer selection");

        let document = ProjectDocument::from_gds(path, &[selection]).expect("import document");
        assert!(
            document.cells.len() > 1,
            "expected preserved cell definitions"
        );
        assert!(
            document
                .cells
                .values()
                .any(|cell| !cell.instances.is_empty())
        );

        for cell in document.cells.values() {
            let mut source_indices = HashSet::new();
            for shape in cell.shapes.values() {
                assert_eq!(shape.parent_cell, cell.id);
                let source = document
                    .source_map
                    .get(&shape.id)
                    .expect("shape source map entry");
                assert_eq!(source.cell_name, cell.name);
                assert!(
                    source_indices.insert(source.element_index),
                    "a GDS element can only map to one imported shape"
                );
            }
            for instance in cell.instances.values() {
                assert_eq!(instance.parent_cell, cell.id);
                assert!(document.cells.contains_key(&instance.cell_id));
            }
        }

        let render_scene = document
            .compile_render_scene()
            .expect("compile render scene");
        assert!(!render_scene.layers.is_empty());
        for layer in &render_scene.layers {
            assert_eq!(layer.object.polygons.len(), layer.occurrences.len());
            assert!(layer.occurrences.iter().all(|occurrence| {
                document.cells.contains_key(&occurrence.leaf_cell)
                    && document.source_map.contains_key(&occurrence.shape_id)
            }));
            for occurrence in &layer.occurrences {
                let inspection = document
                    .inspect_occurrence(occurrence)
                    .expect("inspect compiled occurrence");
                assert_eq!(inspection.shape_id, occurrence.shape_id);
                assert_eq!(inspection.layer, layer.object.layer);
                assert_eq!(inspection.datatype, layer.object.datatype);
                assert_eq!(
                    inspection.hierarchy_path.last(),
                    Some(&inspection.cell_name),
                    "hierarchy path must end at the shape-owning cell"
                );
            }
        }

        let rebuilt_scene = document
            .compile_render_scene()
            .expect("rebuild render scene");
        assert_eq!(
            render_scene.objects(),
            rebuilt_scene.objects(),
            "rebuilding the disposable render cache must preserve the visible scene"
        );
    }

    #[test]
    fn inspects_top_cells() {
        let info = inspect_gds_file(Path::new("../models/AWG_0.8nmCS_16CH_0nmOS.gds"))
            .expect("inspect sample GDS");

        assert!(info.cells.iter().any(|cell| cell.name == "AWG"));
        assert!(
            !info
                .cells
                .iter()
                .any(|cell| cell.name.starts_with("straight_gdsfactory"))
        );

        let awg = info
            .cells
            .iter()
            .find(|cell| cell.name == "AWG")
            .expect("AWG top cell");
        let layer = awg
            .layers
            .iter()
            .find(|layer| layer.selection.layer == 4 && layer.selection.datatype == 1)
            .expect("AWG L4/1 layer");
        assert_eq!(
            layer.polygon_count, 1,
            "expected near-touching AWG geometry to collapse into one connected island"
        );
    }

    #[test]
    fn unions_overlaps_without_joining_disconnected_polygons() {
        let polygons = vec![
            test_polygon(0.0, 0.0, 2.0, 2.0),
            test_polygon(1.0, 0.0, 3.0, 2.0),
            test_polygon(10.0, 10.0, 11.0, 11.0),
        ];

        let union = union_layer_polygons(polygons);

        assert_eq!(union.len(), 2);
        let total_area = union
            .iter()
            .map(|polygon| {
                super::super::geometry::signed_contour_area(&polygon.points).abs()
                    - polygon
                        .holes
                        .iter()
                        .map(|hole| super::super::geometry::signed_contour_area(hole).abs())
                        .sum::<f64>()
            })
            .sum::<f64>();
        assert!((total_area - 7.0).abs() < f64::EPSILON);
    }

    fn test_polygon(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Polygon2d {
        Polygon2d {
            points: vec![
                [min_x, min_y],
                [max_x, min_y],
                [max_x, max_y],
                [min_x, max_y],
            ],
            holes: Vec::new(),
        }
    }
}

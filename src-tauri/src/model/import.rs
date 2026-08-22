use std::collections::HashSet;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::archive::source_key_for_path;

use super::gds::{LayerGeometry, LayerKey, parse_gds_layers};
use super::render::{BaseplateObject, GdsLayerObject, SceneObject};
use super::types::{Bounds2d, DisplayProperties, ProjectDocument};

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

pub fn new_object_id() -> String {
    Uuid::new_v4().simple().to_string()
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

pub fn import_gds_layers(path: &Path) -> anyhow::Result<Vec<SceneObject>> {
    let parsed = parse_gds_layers(path)?;
    objects_from_layers(path, parsed.flatten_display_layers()?)
}

pub fn import_gds_document(
    path: &Path,
    selections: &[GdsLayerSelection],
) -> anyhow::Result<ProjectDocument> {
    ProjectDocument::from_gds(path, selections)
}

pub fn import_gds_layer_selections(
    path: &Path,
    selections: &[GdsLayerSelection],
) -> anyhow::Result<Vec<SceneObject>> {
    if selections.is_empty() {
        return Ok(Vec::new());
    }

    let parsed = parse_gds_layers(path)?;
    let selected = selections.iter().cloned().collect::<HashSet<_>>();
    let mut layers = IndexMap::new();
    for cell_name in parsed.display_cells() {
        layers.extend(parsed.flatten_cell_layers(&cell_name)?.into_iter().filter(
            |(key, _geometry)| {
                selected.contains(&GdsLayerSelection {
                    cell_name: key.cell_name.clone(),
                    layer: key.layer,
                    datatype: key.datatype,
                })
            },
        ));
    }
    objects_from_layers(path, layers)
}

fn objects_from_layers(
    path: &Path,
    layers: IndexMap<LayerKey, LayerGeometry>,
) -> anyhow::Result<Vec<SceneObject>> {
    let source_key = source_key_for_path(path);
    let mut objects = Vec::new();
    for (key, geometry) in layers {
        objects.push(SceneObject::GdsLayer(GdsLayerObject {
            id: new_object_id(),
            display: DisplayProperties::gds_layer(format!("L{}/{}", key.layer, key.datatype)),
            file_path: path.to_path_buf(),
            source_path: path.to_path_buf(),
            source_key: source_key.clone(),
            cell_name: key.cell_name,
            layer: key.layer,
            datatype: key.datatype,
            bounds: geometry.bounds,
            polygons: geometry.polygons,
        }));
    }

    if objects.is_empty() {
        anyhow::bail!("no GDS boundary, path, or box geometry found");
    }

    Ok(objects)
}

pub fn new_baseplate(name: impl Into<String>, bounds: Bounds2d) -> SceneObject {
    SceneObject::Baseplate(BaseplateObject {
        id: new_object_id(),
        display: DisplayProperties::baseplate(name),
        default_bounds: Some(bounds.clone()),
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        import_gds_document, import_gds_layers, inspect_gds_file, new_baseplate, new_object_id,
    };
    use crate::model::{
        Bounds2d, CellKey, DisplayProperties, GdsLayerObject, Polygon2d, Scene, SceneObject,
        Selection, union_layer_polygons,
    };

    #[test]
    fn imports_gds_layers() {
        let objects = import_gds_layers(Path::new("../models/AWG.gds")).expect("import sample GDS");
        assert!(!objects.is_empty());
        for obj in objects {
            let SceneObject::GdsLayer(layer) = obj else {
                panic!("expected GDS layer object");
            };
            assert!(!layer.cell_name.starts_with("$$$"));
            assert!(!layer.polygons.is_empty());
            assert!(layer.bounds.min_x < layer.bounds.max_x);
            assert!(layer.bounds.min_y < layer.bounds.max_y);
        }
    }

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

        let document = import_gds_document(path, &[selection]).expect("import document");
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
            for shape in cell.shapes.values() {
                assert_eq!(shape.parent_cell, cell.id);
                assert!(document.source_map.contains_key(&shape.id));
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
        for layer in render_scene.layers {
            assert_eq!(layer.object.polygons.len(), layer.occurrences.len());
            assert!(layer.occurrences.iter().all(|occurrence| {
                document.cells.contains_key(&occurrence.leaf_cell)
                    && document.source_map.contains_key(&occurrence.shape_id)
            }));
        }
    }

    #[test]
    fn stable_baseplate_names() {
        let mut scene = Scene::default();
        assert_eq!(scene.next_baseplate_name(), "Baseplate 1");

        let bounds = Bounds2d {
            min_x: -1.0,
            min_y: -1.0,
            max_x: 1.0,
            max_y: 1.0,
        };
        scene
            .add(new_baseplate("Baseplate 1", bounds.clone()))
            .expect("add first baseplate");
        scene
            .add(new_baseplate("Baseplate 3", bounds))
            .expect("add third baseplate");

        assert_eq!(scene.next_baseplate_name(), "Baseplate 2");
    }

    #[test]
    fn bounds_from_cell_selection() {
        let mut scene = Scene::default();
        let file_path = PathBuf::from("models/test.gds");
        scene
            .add(test_gds_layer(
                file_path.clone(),
                "AWG",
                Bounds2d {
                    min_x: -10.0,
                    min_y: -20.0,
                    max_x: 30.0,
                    max_y: 40.0,
                },
            ))
            .expect("add selected cell layer");
        scene
            .add(test_gds_layer(
                file_path.clone(),
                "Other",
                Bounds2d {
                    min_x: -1000.0,
                    min_y: -1000.0,
                    max_x: 1000.0,
                    max_y: 1000.0,
                },
            ))
            .expect("add other cell layer");

        let bounds = scene.default_baseplate_bounds(&Selection::Cell(CellKey {
            file_path,
            cell_name: "AWG".to_owned(),
        }));

        assert_eq!(
            bounds,
            Bounds2d {
                min_x: -10.0,
                min_y: -20.0,
                max_x: 30.0,
                max_y: 40.0,
            }
        );
    }

    #[test]
    fn bounds_from_scene_selection() {
        let mut scene = Scene::default();
        scene
            .add(test_gds_layer(
                PathBuf::from("models/a.gds"),
                "A",
                Bounds2d {
                    min_x: -1.0,
                    min_y: -2.0,
                    max_x: 3.0,
                    max_y: 4.0,
                },
            ))
            .expect("add first layer");
        scene
            .add(test_gds_layer(
                PathBuf::from("models/b.gds"),
                "B",
                Bounds2d {
                    min_x: -10.0,
                    min_y: 5.0,
                    max_x: 20.0,
                    max_y: 30.0,
                },
            ))
            .expect("add second layer");

        let bounds = scene.default_baseplate_bounds(&Selection::Scene);

        assert_eq!(
            bounds,
            Bounds2d {
                min_x: -10.0,
                min_y: -2.0,
                max_x: 20.0,
                max_y: 30.0,
            }
        );
    }

    #[test]
    fn bounds_ignore_baseplates() {
        let mut scene = Scene::default();
        scene
            .add(new_baseplate(
                "Baseplate 1",
                Bounds2d {
                    min_x: -1000.0,
                    min_y: -1000.0,
                    max_x: 1000.0,
                    max_y: 1000.0,
                },
            ))
            .expect("add existing baseplate");

        let bounds = scene.default_baseplate_bounds(&Selection::Scene);

        assert_eq!(
            bounds,
            Bounds2d {
                min_x: -100.0,
                min_y: -100.0,
                max_x: 100.0,
                max_y: 100.0,
            }
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
            layer.polygon_count, 2,
            "expected overlapping AWG geometry to collapse to its two disconnected islands"
        );

        let objects = import_gds_layers(Path::new("../models/AWG_0.8nmCS_16CH_0nmOS.gds"))
            .expect("import AWG regression model");
        let imported_layer = objects
            .iter()
            .find_map(|object| match object {
                SceneObject::GdsLayer(layer) if layer.layer == 4 && layer.datatype == 1 => {
                    Some(layer)
                }
                _ => None,
            })
            .expect("imported AWG L4/1 layer");
        assert_eq!(imported_layer.polygons.len(), 2);
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

    fn test_gds_layer(file_path: PathBuf, cell_name: &str, bounds: Bounds2d) -> SceneObject {
        SceneObject::GdsLayer(GdsLayerObject {
            id: new_object_id(),
            display: DisplayProperties::gds_layer("L4/1"),
            file_path: file_path.clone(),
            source_path: file_path,
            source_key: String::new(),
            cell_name: cell_name.to_owned(),
            layer: 4,
            datatype: 1,
            bounds,
            polygons: Vec::new(),
        })
    }
}

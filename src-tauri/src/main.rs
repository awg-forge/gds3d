#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod archive;
// The scene module retains APIs used by the next editor features (baseplates,
// selection and partial GDS imports). They are intentionally kept while the
// Svelte editor is migrated from the egui application.
#[allow(dead_code)]
mod model;

use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

#[derive(Default)]
struct SceneState(Mutex<model::Scene>);

#[derive(Serialize)]
struct SceneSnapshot {
    revision: u64,
    objects: Vec<model::SceneObject>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisplayUpdate {
    object_id: String,
    color: Option<String>,
    brightness: Option<f32>,
    visible: Option<bool>,
    z_min: Option<f32>,
    z_max: Option<f32>,
}

fn snapshot(scene: &model::Scene) -> SceneSnapshot {
    SceneSnapshot {
        revision: scene.revision(),
        objects: scene.objects().cloned().collect(),
    }
}

#[tauri::command]
fn inspect_gds_file(path: String) -> Result<model::GdsFileInfo, String> {
    model::inspect_gds_file(Path::new(&path)).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_gds(path: String, state: tauri::State<'_, SceneState>) -> Result<SceneSnapshot, String> {
    let objects = model::import_gds_layers(Path::new(&path)).map_err(|error| error.to_string())?;
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    *scene = model::Scene::default();
    for object in objects {
        scene.add(object).map_err(|error| error.to_string())?;
    }
    Ok(snapshot(&scene))
}

#[tauri::command]
fn scene_snapshot(state: tauri::State<'_, SceneState>) -> Result<SceneSnapshot, String> {
    let scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    Ok(snapshot(&scene))
}

#[tauri::command]
fn update_object_display(
    update: DisplayUpdate,
    state: tauri::State<'_, SceneState>,
) -> Result<SceneSnapshot, String> {
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    let object = scene
        .get_mut(&update.object_id)
        .ok_or_else(|| "scene object not found".to_owned())?;
    let display = object.display_mut();
    if let Some(color) = update.color {
        display.color = color;
    }
    if let Some(brightness) = update.brightness {
        display.brightness = brightness.clamp(0.05, 2.0);
    }
    if let Some(visible) = update.visible {
        display.visible = visible;
    }
    if let Some(z_min) = update.z_min {
        display.z_min = z_min;
    }
    if let Some(z_max) = update.z_max {
        display.z_max = z_max;
    }
    scene.touch();
    Ok(snapshot(&scene))
}

#[tauri::command]
fn save_project(path: String, state: tauri::State<'_, SceneState>) -> Result<(), String> {
    let scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    archive::write_archive(
        Path::new(&path),
        &scene.objects().cloned().collect::<Vec<_>>(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_project(
    path: String,
    state: tauri::State<'_, SceneState>,
) -> Result<SceneSnapshot, String> {
    let objects =
        archive::read_scene_objects(Path::new(&path)).map_err(|error| error.to_string())?;
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    *scene = model::Scene::default();
    for object in objects {
        scene.add(object).map_err(|error| error.to_string())?;
    }
    Ok(snapshot(&scene))
}

fn main() {
    tauri::Builder::default()
        .manage(SceneState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            inspect_gds_file,
            import_gds,
            scene_snapshot,
            update_object_display,
            save_project,
            load_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gds3d");
}

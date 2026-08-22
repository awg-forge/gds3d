#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod archive;
mod export;
// The scene module retains APIs used by the next editor features (baseplates,
// selection and partial GDS imports). They are intentionally kept while the
// Svelte editor is migrated from the egui application.
#[allow(dead_code)]
mod model;

use serde::Serialize;
use std::collections::BTreeSet;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Window as TauriWindow, WindowEvent, Wry};
use tauri_plugin_window_state::{StateFlags, WindowExt};

#[derive(Default)]
struct SceneState(Arc<Mutex<model::ProjectDocument>>);

const MAIN_WINDOW_LABEL: &str = "main";
const PREFERENCES_FILENAME: &str = "desktop-preferences.json";
const TRAY_SHOW_ID: &str = "tray-show";
const TRAY_QUIT_ID: &str = "tray-quit";
static SYSTEM_FONTS: OnceLock<Vec<String>> = OnceLock::new();

struct TrayMenuState {
    show: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

struct TrayLabels {
    show: &'static str,
    quit: &'static str,
}

fn tray_labels(locale: &str) -> TrayLabels {
    if locale == "zh-CN" {
        TrayLabels {
            show: "显示主窗口",
            quit: "退出",
        }
    } else {
        TrayLabels {
            show: "Show Main Window",
            quit: "Quit",
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn gds3d_install_window_style(window: *mut c_void) -> i32;
}

#[cfg(target_os = "macos")]
fn install_macos_window_style(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is unavailable".to_owned())?;
    let native_window = window.ns_window().map_err(|error| error.to_string())?;
    if unsafe { gds3d_install_window_style(native_window) } == 1 {
        Ok(())
    } else {
        Err("failed to install macOS window style".to_owned())
    }
}

#[derive(Clone, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopPreferences {
    remember_window_state: bool,
    close_to_tray: bool,
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            remember_window_state: true,
            close_to_tray: false,
        }
    }
}

struct DesktopPreferencesState {
    path: PathBuf,
    preferences: Mutex<DesktopPreferences>,
}

impl DesktopPreferencesState {
    fn load(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = app.path().app_config_dir()?;
        let path = directory.join(PREFERENCES_FILENAME);
        let preferences = match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents).unwrap_or_default(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                DesktopPreferences::default()
            }
            Err(error) => return Err(Box::new(error)),
        };
        Ok(Self {
            path,
            preferences: Mutex::new(preferences),
        })
    }

    fn preferences(&self) -> Result<DesktopPreferences, String> {
        self.preferences
            .lock()
            .map(|preferences| preferences.clone())
            .map_err(|_| "desktop preferences are unavailable".to_owned())
    }

    fn update(&self, preferences: DesktopPreferences) -> Result<(), String> {
        let serialized =
            serde_json::to_vec_pretty(&preferences).map_err(|error| error.to_string())?;
        let directory = self
            .path
            .parent()
            .ok_or_else(|| "desktop preferences path has no parent directory".to_owned())?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        fs::write(&self.path, serialized).map_err(|error| error.to_string())?;
        *self
            .preferences
            .lock()
            .map_err(|_| "desktop preferences are unavailable".to_owned())? = preferences;
        Ok(())
    }
}

fn window_state_flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED
}

fn setup_tray(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let labels = tray_labels("en");
    let show = MenuItem::with_id(app, TRAY_SHOW_ID, labels.show, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    app.manage(TrayMenuState {
        show: show.clone(),
        quit: quit.clone(),
    });
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| std::io::Error::other("default application icon is unavailable"))?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("gds3d")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW_ID => show_main_window(app),
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[tauri::command]
fn update_tray_locale(app: AppHandle, locale: String) -> Result<(), String> {
    let labels = tray_labels(&locale);
    let state = app.state::<TrayMenuState>();
    state
        .show
        .set_text(labels.show)
        .map_err(|error| error.to_string())?;
    state
        .quit
        .set_text(labels.quit)
        .map_err(|error| error.to_string())
}

fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
fn frontend_ready(app: AppHandle) {
    show_main_window(&app);
}

fn handle_window_event(window: &TauriWindow<Wry>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event
        && window
            .state::<DesktopPreferencesState>()
            .preferences()
            .is_ok_and(|preferences| preferences.close_to_tray)
    {
        api.prevent_close();
        let _ = window.hide();
    }
}

#[derive(Serialize)]
struct SceneSnapshot {
    revision: u64,
    objects: Vec<model::SceneObject>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisplayUpdate {
    object_id: String,
    name: Option<String>,
    color: Option<String>,
    opacity: Option<f32>,
    visible: Option<bool>,
    z_min: Option<f32>,
    z_max: Option<f32>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaseplateTarget {
    file_path: String,
    cell_name: String,
}

const MINIMUM_Z_SPAN: f32 = 1.0;
const Z_SPAN_TOLERANCE: f32 = 0.0001;

fn snapshot(document: &model::ProjectDocument) -> Result<SceneSnapshot, String> {
    Ok(SceneSnapshot {
        revision: document.revision(),
        objects: document
            .compile_render_scene()
            .map_err(|error| error.to_string())?
            .objects(),
    })
}

#[tauri::command]
async fn inspect_gds_file(path: String) -> Result<model::GdsFileInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        model::inspect_gds_file(Path::new(&path)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn import_gds(
    path: String,
    selections: Vec<model::GdsLayerSelection>,
    state: tauri::State<'_, SceneState>,
) -> Result<SceneSnapshot, String> {
    if selections.is_empty() {
        return Err("select at least one GDS layer".to_owned());
    }
    let document_state = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut document = document_state
            .lock()
            .map_err(|_| "scene state is unavailable".to_owned())?;
        document
            .import_gds(Path::new(&path), &selections)
            .map_err(|error| error.to_string())?;
        snapshot(&document)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn scene_snapshot(state: tauri::State<'_, SceneState>) -> Result<SceneSnapshot, String> {
    let scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    snapshot(&scene)
}

#[tauri::command]
fn clear_scene(state: tauri::State<'_, SceneState>) -> Result<SceneSnapshot, String> {
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    *scene = model::ProjectDocument::default();
    snapshot(&scene)
}

#[tauri::command]
fn update_object_display(
    update: DisplayUpdate,
    state: tauri::State<'_, SceneState>,
) -> Result<(), String> {
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    let display = scene
        .display_mut(&update.object_id)
        .ok_or_else(|| "scene object not found".to_owned())?;
    let z_min = update.z_min.unwrap_or(display.z_min);
    let z_max = update.z_max.unwrap_or(display.z_max);
    if z_max - z_min < MINIMUM_Z_SPAN - Z_SPAN_TOLERANCE {
        return Err("Z range must be at least 1".to_owned());
    }
    if let Some(name) = update.name {
        let name = name.trim();
        if name.is_empty() {
            return Err("display name cannot be empty".to_owned());
        }
        display.name = name.to_owned();
    }
    if let Some(color) = update.color {
        display.color = color;
    }
    if let Some(opacity) = update.opacity {
        display.opacity = opacity.clamp(0.0, 1.0);
    }
    if let Some(visible) = update.visible {
        display.visible = visible;
    }
    if update.z_min.is_some() {
        display.z_min = z_min;
    }
    if update.z_max.is_some() {
        display.z_max = z_max;
    }
    scene.touch();
    Ok(())
}

#[tauri::command]
fn set_objects_visibility(
    object_ids: Vec<String>,
    visible: bool,
    state: tauri::State<'_, SceneState>,
) -> Result<(), String> {
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    for object_id in object_ids {
        let display = scene
            .display_mut(&object_id)
            .ok_or_else(|| format!("scene object not found: {object_id}"))?;
        display.visible = visible;
    }
    scene.touch();
    Ok(())
}

#[tauri::command]
fn create_baseplate(
    target: Option<BaseplateTarget>,
    state: tauri::State<'_, SceneState>,
) -> Result<SceneSnapshot, String> {
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    let selection = target.map_or(model::Selection::Scene, |target| {
        model::Selection::Cell(model::CellKey {
            file_path: PathBuf::from(target.file_path),
            cell_name: target.cell_name,
        })
    });
    let bounds = scene
        .default_baseplate_bounds(&selection)
        .map_err(|error| error.to_string())?;
    let baseplate_name = scene.next_baseplate_name();
    scene.add_baseplate(baseplate_name, bounds);
    snapshot(&scene)
}

#[tauri::command]
fn delete_scene_object(
    object_id: String,
    state: tauri::State<'_, SceneState>,
) -> Result<SceneSnapshot, String> {
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    if !scene.remove_render_object(&object_id) {
        return Err("scene object not found".to_owned());
    }
    snapshot(&scene)
}

#[tauri::command]
fn get_desktop_preferences(
    state: tauri::State<'_, DesktopPreferencesState>,
) -> Result<DesktopPreferences, String> {
    state.preferences()
}

#[tauri::command]
fn update_desktop_preferences(
    preferences: DesktopPreferences,
    state: tauri::State<'_, DesktopPreferencesState>,
) -> Result<(), String> {
    state.update(preferences)
}

#[tauri::command]
async fn get_system_fonts() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        SYSTEM_FONTS
            .get_or_init(|| {
                let mut database = fontdb::Database::new();
                database.load_system_fonts();
                database
                    .faces()
                    .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            })
            .clone()
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_project(path: String, state: tauri::State<'_, SceneState>) -> Result<(), String> {
    let scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    archive::write_project_archive(Path::new(&path), &scene).map_err(|error| error.to_string())
}

#[tauri::command]
fn load_project(
    path: String,
    state: tauri::State<'_, SceneState>,
) -> Result<SceneSnapshot, String> {
    let document =
        archive::read_project_document(Path::new(&path)).map_err(|error| error.to_string())?;
    let mut scene = state
        .0
        .lock()
        .map_err(|_| "scene state is unavailable".to_owned())?;
    *scene = document;
    snapshot(&scene)
}

fn main() {
    tauri::Builder::default()
        .manage(SceneState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(window_state_flags())
                .skip_initial_state(MAIN_WINDOW_LABEL)
                .build(),
        )
        .setup(|app| {
            let preferences = DesktopPreferencesState::load(app.handle())?;
            let remembers_window_state = preferences
                .preferences()
                .unwrap_or_default()
                .remember_window_state;
            app.manage(preferences);
            if remembers_window_state
                && let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL)
            {
                window.restore_state(window_state_flags())?;
            }
            #[cfg(target_os = "macos")]
            if let Err(error) = install_macos_window_style(app.handle()) {
                eprintln!("{error}");
            }
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            frontend_ready,
            inspect_gds_file,
            import_gds,
            scene_snapshot,
            clear_scene,
            update_object_display,
            set_objects_visibility,
            create_baseplate,
            delete_scene_object,
            save_project,
            load_project,
            export::export_view,
            export::export_model,
            get_desktop_preferences,
            update_desktop_preferences,
            update_tray_locale,
            get_system_fonts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gds3d");
}

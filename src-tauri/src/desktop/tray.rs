use super::autodelay::AutoDelay;
use super::lightweight;
use super::window_state::{
    self as window_lifecycle, MAIN_WINDOW_LABEL, MainWindowMode, MainWindowState,
};
use crate::settings::SettingsState;
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Window as TauriWindow, WindowEvent, Wry};

const SHOW_MENU_ID: &str = "tray-show";
const LIGHTWEIGHT_MENU_ID: &str = "tray-lightweight";
const QUIT_MENU_ID: &str = "tray-quit";

struct TrayMenuState {
    show: MenuItem<Wry>,
    lightweight: CheckMenuItem<Wry>,
    quit: MenuItem<Wry>,
}

#[derive(Debug, PartialEq, Eq)]
struct TrayLabels {
    show: &'static str,
    lightweight: &'static str,
    quit: &'static str,
}

fn tray_labels(locale: &str) -> TrayLabels {
    if locale == "zh-CN" {
        TrayLabels {
            show: "显示主窗口",
            lightweight: "轻量模式",
            quit: "退出",
        }
    } else {
        TrayLabels {
            show: "Show Main Window",
            lightweight: "Lightweight Mode",
            quit: "Quit",
        }
    }
}

pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let labels = tray_labels(&app.state::<SettingsState>().locale());
    let show = MenuItem::with_id(app, SHOW_MENU_ID, labels.show, true, None::<&str>)?;
    let lightweight = CheckMenuItem::with_id(
        app,
        LIGHTWEIGHT_MENU_ID,
        labels.lightweight,
        true,
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, QUIT_MENU_ID, labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &lightweight, &quit])?;
    app.state::<MainWindowState>()
        .set_tray_item(lightweight.clone());
    app.manage(TrayMenuState {
        show: show.clone(),
        lightweight: lightweight.clone(),
        quit: quit.clone(),
    });
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| std::io::Error::other("default application icon is unavailable"))?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("SeaLantern Connect")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_MENU_ID => show_main_window(app),
            LIGHTWEIGHT_MENU_ID => {
                if let Err(error) = toggle_lightweight_mode(app) {
                    log::error!("lightweight toggle failed: {error}");
                }
            }
            QUIT_MENU_ID => {
                let _ = app.state::<SettingsState>().persist();
                app.exit(0);
            }
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

pub fn update_locale(app: &AppHandle) -> Result<(), String> {
    let labels = tray_labels(&app.state::<SettingsState>().locale());
    let state = app.state::<TrayMenuState>();
    state
        .show
        .set_text(labels.show)
        .map_err(|error| error.to_string())?;
    state
        .lightweight
        .set_text(labels.lightweight)
        .map_err(|error| error.to_string())?;
    state
        .quit
        .set_text(labels.quit)
        .map_err(|error| error.to_string())
}

pub fn handle_window_event(window: &TauriWindow, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }
    let settings = window.state::<SettingsState>();
    if let WindowEvent::CloseRequested { api, .. } = event {
        let _ = settings.persist();
        api.prevent_close();
        if let Err(error) = window_lifecycle::hide(window.app_handle()) {
            log::error!("window hide failed: {error}");
        } else {
            schedule_auto_lightweight(window.app_handle());
        }
    }
}

pub(crate) fn show_main_window(app: &AppHandle) {
    app.state::<AutoDelay>().cancel();
    if let Err(error) = reveal_main_window(app) {
        log::error!("tray show failed: {error}");
    }
}

pub(crate) fn show_when_ready(app: &AppHandle) {
    if app.state::<MainWindowState>().mode() == MainWindowMode::Hidden {
        show_main_window(app);
    }
}

pub(crate) fn start_silently(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<MainWindowState>();
    let mut transition = state.begin_transition()?;
    lightweight::enter(app, &mut transition)
}

fn reveal_main_window(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<MainWindowState>();
    let mut transition = state.begin_transition()?;
    if app.get_webview_window(MAIN_WINDOW_LABEL).is_none()
        && transition.mode() != MainWindowMode::Background
    {
        transition.move_to(MainWindowMode::Background)?;
    }
    if transition.mode() == MainWindowMode::Background {
        return lightweight::leave(app, &mut transition);
    }
    window_lifecycle::show(app, &mut transition)
}

fn toggle_lightweight_mode(app: &AppHandle) -> Result<(), String> {
    app.state::<AutoDelay>().cancel();
    let state = app.state::<MainWindowState>();
    let mut transition = state.begin_transition()?;
    let leaving = transition.mode() == MainWindowMode::Background;
    if leaving {
        lightweight::leave(app, &mut transition)?;
    } else {
        lightweight::enter(app, &mut transition)?;
    }
    drop(transition);
    if leaving {
        schedule_auto_lightweight(app);
    }
    Ok(())
}

pub(crate) fn schedule_auto_lightweight(app: &AppHandle) {
    let timer = app.state::<AutoDelay>();
    let Some(delay) = app.state::<SettingsState>().auto_lightweight_delay() else {
        timer.cancel();
        return;
    };
    if app.state::<MainWindowState>().mode() != MainWindowMode::Hidden {
        timer.cancel();
        return;
    }

    let app = app.clone();
    timer.schedule(delay, move |ticket| {
        let state = app.state::<MainWindowState>();
        let Ok(mut transition) = state.begin_transition() else {
            return;
        };
        if !ticket.is_current() || transition.mode() != MainWindowMode::Hidden {
            return;
        }
        app.state::<AutoDelay>().cancel();
        if let Err(error) = lightweight::enter(&app, &mut transition) {
            log::error!("automatic lightweight entry failed: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localizes_tray_labels() {
        assert_eq!(tray_labels("zh-CN").quit, "退出");
        assert_eq!(tray_labels("en").lightweight, "Lightweight Mode");
        assert_eq!(tray_labels("unknown").show, "Show Main Window");
    }
}

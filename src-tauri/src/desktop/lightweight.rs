use super::theme;
use super::window_state::{
    self as window_lifecycle, MAIN_WINDOW_LABEL, MainWindowMode, MainWindowTransition,
};
use crate::settings::SettingsState;
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::{AppHandle, Manager, WebviewWindowBuilder};
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};

fn window_state_flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED
}

pub(crate) fn leave(
    app: &AppHandle,
    transition: &mut MainWindowTransition<'_>,
) -> Result<(), String> {
    log::debug!(
        "lightweight leave: {:?}, window={}",
        transition.mode(),
        app.get_webview_window(MAIN_WINDOW_LABEL).is_some()
    );
    if transition.mode() != MainWindowMode::Background {
        log::debug!("lightweight leave skipped");
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        app.set_activation_policy(ActivationPolicy::Regular)
            .map_err(|error| error.to_string())?;
    }

    let window = if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window
    } else {
        log::debug!("window rebuild");
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == MAIN_WINDOW_LABEL)
            .ok_or_else(|| "main window configuration is unavailable".to_owned())?;
        WebviewWindowBuilder::from_config(app, config)
            .map_err(|error| error.to_string())?
            .build()
            .map_err(|error| error.to_string())?
    };

    // The state machine requires Background -> Hidden -> Visible.
    window.hide().map_err(|error| error.to_string())?;
    if app.state::<SettingsState>().remembers_window_state() {
        let _ = window.restore_state(window_state_flags());
    }
    let settings = app.state::<SettingsState>();
    let material = settings.window_material();
    let theme = settings.theme();
    log::debug!("material restore: {material}/{theme}");
    if let Err(error) = theme::apply_material(app, &material, &theme, None) {
        log::error!("material restore failed: {error}");
    }
    #[cfg(target_os = "windows")]
    let _ = window.set_skip_taskbar(false);
    transition.move_to(MainWindowMode::Hidden)?;
    window_lifecycle::show(app, transition)
}

pub(crate) fn enter(
    app: &AppHandle,
    transition: &mut MainWindowTransition<'_>,
) -> Result<(), String> {
    log::debug!(
        "lightweight enter: {:?}, window={}",
        transition.mode(),
        app.get_webview_window(MAIN_WINDOW_LABEL).is_some()
    );
    if transition.mode() == MainWindowMode::Background {
        log::debug!("lightweight enter skipped");
        return Ok(());
    }

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = app.save_window_state(window_state_flags());
        #[cfg(target_os = "windows")]
        let _ = window.set_skip_taskbar(true);
        window.destroy().map_err(|error| error.to_string())?;
    }

    transition.move_to(MainWindowMode::Background)?;
    #[cfg(target_os = "macos")]
    {
        app.set_activation_policy(ActivationPolicy::Accessory)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

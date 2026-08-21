use super::theme::SystemTheme;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use super::window_state::MAIN_WINDOW_LABEL;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
use tauri::AppHandle;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri::Manager;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn sealantern_supports_liquid_glass() -> i32;
    fn sealantern_set_liquid_glass(window: *mut c_void, enabled: i32) -> i32;
}

#[cfg(target_os = "macos")]
fn set_liquid_glass(app: &AppHandle, enabled: bool) -> Result<bool, String> {
    if enabled && unsafe { sealantern_supports_liquid_glass() } != 1 {
        log::warn!("liquid glass unavailable");
        return Ok(false);
    }
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        log::warn!("liquid glass skipped: window missing");
        return Ok(true);
    };
    let native_window = window.ns_window().map_err(|error| error.to_string())?;
    // Tauri owns the NSWindow. Swift uses this unowned pointer synchronously on AppKit's main thread.
    let status = unsafe { sealantern_set_liquid_glass(native_window, i32::from(enabled)) };
    if status == 1 {
        log::debug!(
            "liquid glass: {}",
            if enabled { "installed" } else { "removed" }
        );
        Ok(true)
    } else {
        Err("failed to update macOS Liquid Glass".to_owned())
    }
}

pub(crate) fn set_material(
    app: &AppHandle,
    material: &str,
    theme: SystemTheme,
) -> Result<(), String> {
    log::debug!("material: {material}/{theme:?}");
    #[cfg(target_os = "windows")]
    {
        use tauri::window::{Color, Effect, EffectsBuilder};

        let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
            log::warn!("material skipped: window missing");
            return Ok(());
        };

        let effects = match material {
            "mica" => Some(
                EffectsBuilder::new()
                    .effect(match theme {
                        SystemTheme::Dark => Effect::MicaDark,
                        SystemTheme::Light => Effect::MicaLight,
                    })
                    .build(),
            ),
            "acrylic" => Some(
                EffectsBuilder::new()
                    .effect(Effect::Acrylic)
                    .color(match theme {
                        SystemTheme::Dark => Color(32, 32, 32, 225),
                        SystemTheme::Light => Color(245, 245, 245, 215),
                    })
                    .build(),
            ),
            _ => None,
        };
        window
            .set_effects(effects)
            .map_err(|error| error.to_string())
    }

    #[cfg(target_os = "macos")]
    {
        use tauri::window::{Effect, EffectsBuilder};

        let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
            return Ok(());
        };

        if material == "liquid_glass" && set_liquid_glass(app, true)? {
            return Ok(());
        }
        set_liquid_glass(app, false)?;

        let effects = matches!(material, "vibrancy" | "liquid_glass").then(|| {
            EffectsBuilder::new()
                .effect(Effect::UnderWindowBackground)
                .build()
        });
        window
            .set_effects(effects)
            .map_err(|error| error.to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, material, theme);
        Ok(())
    }
}

#[tauri::command]
pub(crate) fn supports_liquid_glass() -> bool {
    #[cfg(target_os = "macos")]
    return unsafe { sealantern_supports_liquid_glass() == 1 };

    #[cfg(not(target_os = "macos"))]
    false
}

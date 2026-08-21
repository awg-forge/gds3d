use super::effects;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use super::window_state::MAIN_WINDOW_LABEL;
use tauri::AppHandle;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use tauri::Manager;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SystemTheme {
    Light,
    Dark,
}

pub(crate) fn apply_material(
    app: &AppHandle,
    material: &str,
    preference: &str,
    system_theme: Option<SystemTheme>,
) -> Result<(), String> {
    let effective = match preference {
        "dark" => SystemTheme::Dark,
        "light" => SystemTheme::Light,
        _ => system_theme.unwrap_or_else(|| current(app)),
    };

    // Install the matching material first so releasing an explicit theme never exposes a white frame.
    effects::set_material(app, material, effective)?;
    set_window(app, preference, effective)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn set_window(app: &AppHandle, preference: &str, effective: SystemTheme) -> Result<(), String> {
    use tauri::Theme;

    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        log::warn!("theme skipped: window missing");
        return Ok(());
    };
    let theme = match preference {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    };
    window.set_theme(theme).map_err(|error| error.to_string())?;
    log::debug!("theme: {preference}/{effective:?}");
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn set_window(_app: &AppHandle, _preference: &str, _effective: SystemTheme) -> Result<(), String> {
    Ok(())
}

pub(crate) fn current(app: &AppHandle) -> SystemTheme {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        windows_theme()
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL)
            && let Ok(theme) = window.theme()
        {
            return match theme {
                tauri::Theme::Dark => SystemTheme::Dark,
                _ => SystemTheme::Light,
            };
        }
        SystemTheme::Light
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        SystemTheme::Light
    }
}

#[cfg(target_os = "windows")]
fn windows_theme() -> SystemTheme {
    use std::mem::MaybeUninit;
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_READ, REG_DWORD, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };

    let key_path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
    let mut key = std::ptr::null_mut();
    let opened =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key_path.as_ptr(), 0, KEY_READ, &mut key) };
    if opened != 0 {
        return SystemTheme::Light;
    }

    let mut value_type = 0;
    let mut value = MaybeUninit::<u32>::uninit();
    let mut value_size = std::mem::size_of::<u32>() as u32;
    let queried = unsafe {
        RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null(),
            &mut value_type,
            value.as_mut_ptr().cast(),
            &mut value_size,
        )
    };
    unsafe { RegCloseKey(key) };
    if queried == 0 && value_type == REG_DWORD && value_size == 4 {
        return if unsafe { value.assume_init() } == 0 {
            SystemTheme::Dark
        } else {
            SystemTheme::Light
        };
    }

    SystemTheme::Light
}

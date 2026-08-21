use crate::desktop::{theme, tray};
use sculk::persist;
use sculk::tunnel::{RelayUrl, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const APP_DIR_NAME: &str = "sealantern-connect";
const PREFERENCES_FILE: &str = "preferences.conf";
const KEY_FILE: &str = "secret.key";
const HOST_STATE_FILE: &str = "host.state";
const DEFAULT_JOIN_PORT: u16 = 25_565;
const SPLASH_DURATION_OPTIONS_MS: [u32; 5] = [0, 500, 1000, 1500, 2000];
const FONT_SIZE_RANGE: std::ops::RangeInclusive<u32> = 12..=20;
const AUTO_LIGHTWEIGHT_MINUTES_RANGE: std::ops::RangeInclusive<u32> = 1..=1440;
pub const RECONNECT_TIMEOUT_OPTIONS_SECS: [u64; 5] = [10, 15, 20, 30, 60];
static SYSTEM_FONTS: OnceLock<Vec<String>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThemeColors {
    bg: String,
    bg_secondary: String,
    bg_tertiary: String,
    primary: String,
    primary_solid: String,
    primary_solid_hover: String,
    secondary: String,
    text_primary: String,
    text_secondary: String,
    border: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CustomTheme {
    light: ThemeColors,
    dark: ThemeColors,
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self {
            light: ThemeColors {
                bg: "#f7f7f6".to_owned(),
                bg_secondary: "#f0f0ef".to_owned(),
                bg_tertiary: "#dedfe0".to_owned(),
                primary: "#45505d".to_owned(),
                primary_solid: "#45505d".to_owned(),
                primary_solid_hover: "#36414d".to_owned(),
                secondary: "#69727c".to_owned(),
                text_primary: "#202326".to_owned(),
                text_secondary: "#666b70".to_owned(),
                border: "#d6d8da".to_owned(),
            },
            dark: ThemeColors {
                bg: "#111214".to_owned(),
                bg_secondary: "#191a1d".to_owned(),
                bg_tertiary: "#25272b".to_owned(),
                primary: "#aab4c0".to_owned(),
                primary_solid: "#455666".to_owned(),
                primary_solid_hover: "#536778".to_owned(),
                secondary: "#c1c7cf".to_owned(),
                text_primary: "#f1f1f2".to_owned(),
                text_secondary: "#a6a8ae".to_owned(),
                border: "#30343a".to_owned(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    theme: String,
    color_theme: String,
    custom_theme: CustomTheme,
    font_size: u32,
    font_family: String,
    splash_duration_ms: u32,
    silent_start: bool,
    auto_update: bool,
    locale: String,
    remember_window_state: bool,
    window_material: String,
    auto_lightweight_minutes: Option<u32>,
    host_uri_lifetime: String,
    join_uri: String,
    join_port: u16,
    reconnect_timeout_secs: Option<u64>,
    relay_custom: bool,
    relay_url: String,
    background_enabled: bool,
    background_image: String,
    background_opacity: f32,
    background_blur: u32,
    background_brightness: f32,
    background_card_blur: u32,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "system".to_owned(),
            color_theme: "default".to_owned(),
            custom_theme: CustomTheme::default(),
            font_size: 14,
            font_family: String::new(),
            splash_duration_ms: 1000,
            silent_start: false,
            auto_update: true,
            locale: "zh-CN".to_owned(),
            remember_window_state: true,
            window_material: "solid".to_owned(),
            auto_lightweight_minutes: None,
            host_uri_lifetime: "always".to_owned(),
            join_uri: String::new(),
            join_port: DEFAULT_JOIN_PORT,
            reconnect_timeout_secs: None,
            relay_custom: false,
            relay_url: String::new(),
            background_enabled: false,
            background_image: String::new(),
            background_opacity: 0.75,
            background_blur: 0,
            background_brightness: 1.0,
            background_card_blur: 8,
        }
    }
}

pub struct SettingsState {
    data_dir: PathBuf,
    path: PathBuf,
    secret_key: SecretKey,
    preferences: Mutex<Preferences>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalizationUpdate {
    theme: String,
    color_theme: String,
    custom_theme: CustomTheme,
    font_size: u32,
    font_family: String,
    window_material: String,
    background_enabled: bool,
    background_image: String,
    background_opacity: f32,
    background_blur: u32,
    background_brightness: f32,
    background_card_blur: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettingsUpdate {
    splash_duration_ms: u32,
    silent_start: bool,
    auto_update: bool,
    remember_window_state: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettingsUpdate {
    relay_custom: bool,
    relay_url: String,
    reconnect_timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightweightSettingsUpdate {
    auto_lightweight_minutes: Option<u32>,
}

impl SettingsState {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let data_dir = app
            .path()
            .data_dir()
            .map_err(|error| error.to_string())?
            .join(APP_DIR_NAME);
        std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
        let path = data_dir.join(PREFERENCES_FILE);
        migrate_legacy_preferences(app, &path)?;
        let preferences = std::fs::read_to_string(&path)
            .map(|content| parse_preferences(&content))
            .unwrap_or_default();
        let secret_key = persist::load_or_generate_key(&data_dir.join(KEY_FILE))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            data_dir,
            path,
            secret_key,
            preferences: Mutex::new(preferences),
        })
    }

    pub fn remember_join_uri(&self, join_uri: String) -> Result<(), String> {
        self.update(|preferences| preferences.join_uri = join_uri)
    }

    pub fn set_join_port(&self, port: u16) -> Result<(), String> {
        if port == 0 {
            return Err("local port must be between 1 and 65535".to_owned());
        }
        self.update(|preferences| preferences.join_port = port)
    }

    pub fn host_secret_key(&self) -> SecretKey {
        self.secret_key.clone()
    }

    pub fn host_state_path(&self) -> PathBuf {
        self.data_dir.join(HOST_STATE_FILE)
    }

    pub fn relay_url(&self) -> Result<Option<RelayUrl>, String> {
        let preferences = self
            .preferences
            .lock()
            .map_err(|_| "settings state is unavailable".to_owned())?;
        if !preferences.relay_custom {
            return Ok(None);
        }
        preferences
            .relay_url
            .trim()
            .parse::<RelayUrl>()
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub fn reconnect_timeout(&self) -> Result<Option<Duration>, String> {
        self.preferences
            .lock()
            .map(|preferences| preferences.reconnect_timeout_secs.map(Duration::from_secs))
            .map_err(|_| "settings state is unavailable".to_owned())
    }

    pub fn remembers_window_state(&self) -> bool {
        self.preferences
            .lock()
            .is_ok_and(|preferences| preferences.remember_window_state)
    }

    pub fn starts_silently(&self) -> bool {
        self.preferences
            .lock()
            .is_ok_and(|preferences| preferences.silent_start)
    }

    pub fn window_material(&self) -> String {
        self.preferences
            .lock()
            .map(|preferences| preferences.window_material.clone())
            .unwrap_or_else(|_| "solid".to_owned())
    }

    pub fn theme(&self) -> String {
        self.preferences
            .lock()
            .map(|preferences| preferences.theme.clone())
            .unwrap_or_else(|_| "system".to_owned())
    }

    pub fn auto_lightweight_delay(&self) -> Option<Duration> {
        self.preferences
            .lock()
            .ok()
            .and_then(|preferences| preferences.auto_lightweight_minutes)
            .map(|minutes| Duration::from_secs(u64::from(minutes) * 60))
    }

    pub fn locale(&self) -> String {
        self.preferences
            .lock()
            .map(|preferences| preferences.locale.clone())
            .unwrap_or_else(|_| "en".to_owned())
    }

    pub fn persist(&self) -> Result<(), String> {
        let preferences = self
            .preferences
            .lock()
            .map_err(|_| "settings state is unavailable".to_owned())?;
        save_preferences(&self.path, &preferences)
    }

    fn update(&self, apply: impl FnOnce(&mut Preferences)) -> Result<(), String> {
        let mut preferences = self
            .preferences
            .lock()
            .map_err(|_| "settings state is unavailable".to_owned())?;
        apply(&mut preferences);
        save_preferences(&self.path, &preferences)
    }
}

fn migrate_legacy_preferences(app: &AppHandle, destination: &PathBuf) -> Result<(), String> {
    if destination.exists() {
        return Ok(());
    }
    let source = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join(PREFERENCES_FILE);
    if source.exists() {
        std::fs::copy(source, destination).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_preferences(state: State<'_, SettingsState>) -> Result<Preferences, String> {
    state
        .preferences
        .lock()
        .map(|preferences| preferences.clone())
        .map_err(|_| "settings state is unavailable".to_owned())
}

#[tauri::command]
pub async fn get_system_fonts() -> Result<Vec<String>, String> {
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
pub fn set_theme(
    theme: String,
    system_theme: Option<theme::SystemTheme>,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err("invalid theme preference".to_owned());
    }
    let window_material = state.window_material();
    state.update(|preferences| preferences.theme = theme.clone())?;
    if let Err(error) = theme::apply_material(&app, &window_material, &theme, system_theme) {
        log::error!("native material update failed: {error}");
    }
    Ok(())
}

#[tauri::command]
pub fn get_system_theme(app: AppHandle) -> theme::SystemTheme {
    theme::current(&app)
}

#[tauri::command]
pub fn set_color_theme(color_theme: String, state: State<'_, SettingsState>) -> Result<(), String> {
    let color_theme = normalize_color_theme(&color_theme).to_owned();
    state.update(|preferences| preferences.color_theme = color_theme)
}

#[tauri::command]
pub fn set_locale(
    locale: String,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if !matches!(locale.as_str(), "zh-CN" | "en") {
        return Err("invalid locale preference".to_owned());
    }
    state.update(|preferences| preferences.locale = locale)?;
    if let Err(error) = tray::update_locale(&app) {
        log::error!("tray locale update failed: {error}");
    }
    Ok(())
}

#[tauri::command]
pub fn set_invite_lifetime(
    lifetime: String,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if !is_invite_lifetime(&lifetime) {
        return Err("invalid host URI lifetime".to_owned());
    }
    state.update(|preferences| preferences.host_uri_lifetime = lifetime)
}

#[tauri::command]
pub fn set_personalization(
    update: PersonalizationUpdate,
    system_theme: Option<theme::SystemTheme>,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if !matches!(update.theme.as_str(), "system" | "light" | "dark") {
        return Err("invalid theme preference".to_owned());
    }
    let color_theme = normalize_color_theme(&update.color_theme).to_owned();
    if !is_custom_theme(&update.custom_theme) {
        return Err("invalid custom theme".to_owned());
    }
    if !FONT_SIZE_RANGE.contains(&update.font_size) {
        return Err("invalid font size".to_owned());
    }
    let font_family = normalize_font_family(&update.font_family)
        .ok_or_else(|| "invalid font family".to_owned())?;
    if !is_window_material(&update.window_material) {
        return Err("invalid window material".to_owned());
    }
    if !(0.0..=1.0).contains(&update.background_opacity)
        || !(0.5..=1.5).contains(&update.background_brightness)
        || update.background_blur > 20
        || !(8..=30).contains(&update.background_card_blur)
    {
        return Err("invalid background settings".to_owned());
    }
    let theme = update.theme.clone();
    let window_material = update.window_material;
    state.update(|preferences| {
        preferences.theme = update.theme;
        preferences.color_theme = color_theme;
        preferences.custom_theme = update.custom_theme;
        preferences.font_size = update.font_size;
        preferences.font_family = font_family;
        preferences.window_material = window_material.clone();
        preferences.background_enabled = update.background_enabled;
        preferences.background_image = update.background_image;
        preferences.background_opacity = update.background_opacity;
        preferences.background_blur = update.background_blur;
        preferences.background_brightness = update.background_brightness;
        preferences.background_card_blur = update.background_card_blur;
    })?;
    if let Err(error) = theme::apply_material(&app, &window_material, &theme, system_theme) {
        log::error!("window material update failed: {error}");
    }
    Ok(())
}

#[tauri::command]
pub fn set_application_settings(
    update: ApplicationSettingsUpdate,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if !SPLASH_DURATION_OPTIONS_MS.contains(&update.splash_duration_ms) {
        return Err("invalid splash duration".to_owned());
    }
    state.update(|preferences| {
        preferences.splash_duration_ms = update.splash_duration_ms;
        preferences.silent_start = update.silent_start;
        preferences.auto_update = update.auto_update;
        preferences.remember_window_state = update.remember_window_state;
    })
}

#[tauri::command]
pub fn set_connection_settings(
    update: ConnectionSettingsUpdate,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if let Some(timeout) = update.reconnect_timeout_secs
        && !RECONNECT_TIMEOUT_OPTIONS_SECS.contains(&timeout)
    {
        return Err("invalid reconnect timeout".to_owned());
    }
    let relay_url = update.relay_url.trim().to_owned();
    if update.relay_custom {
        relay_url
            .parse::<RelayUrl>()
            .map_err(|error| error.to_string())?;
    }
    state.update(|preferences| {
        preferences.relay_custom = update.relay_custom;
        preferences.relay_url = relay_url;
        preferences.reconnect_timeout_secs = update.reconnect_timeout_secs;
    })
}

#[tauri::command]
pub fn set_lightweight_settings(
    update: LightweightSettingsUpdate,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if let Some(minutes) = update.auto_lightweight_minutes
        && !AUTO_LIGHTWEIGHT_MINUTES_RANGE.contains(&minutes)
    {
        return Err("invalid automatic lightweight delay".to_owned());
    }
    state.update(|preferences| {
        preferences.auto_lightweight_minutes = update.auto_lightweight_minutes;
    })?;
    tray::schedule_auto_lightweight(&app);
    Ok(())
}

#[tauri::command]
pub fn set_join_port(port: u16, state: State<'_, SettingsState>) -> Result<(), String> {
    state.set_join_port(port)
}

fn save_preferences(path: &Path, preferences: &Preferences) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "settings directory is unavailable".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let content = format!(
        "theme={}\ncolor_theme={}\ncustom_theme={}\nfont_size={}\nfont_family={}\nsplash_duration_ms={}\nsilent_start={}\nauto_update={}\nlocale={}\nremember_window_state={}\nwindow_material={}\nauto_lightweight_minutes={}\nhost_uri_lifetime={}\njoin_uri={}\njoin_port={}\nreconnect_timeout_secs={}\nrelay_custom={}\nrelay_url={}\nbackground_enabled={}\nbackground_image={}\nbackground_opacity={}\nbackground_blur={}\nbackground_brightness={}\nbackground_card_blur={}\n",
        preferences.theme,
        preferences.color_theme,
        serde_json::to_string(&preferences.custom_theme).map_err(|error| error.to_string())?,
        preferences.font_size,
        preferences.font_family,
        preferences.splash_duration_ms,
        preferences.silent_start,
        preferences.auto_update,
        preferences.locale,
        preferences.remember_window_state,
        preferences.window_material,
        preferences
            .auto_lightweight_minutes
            .map_or_else(|| "off".to_owned(), |value| value.to_string()),
        preferences.host_uri_lifetime,
        preferences.join_uri,
        preferences.join_port,
        preferences
            .reconnect_timeout_secs
            .map_or_else(|| "unlimited".to_owned(), |value| value.to_string()),
        preferences.relay_custom,
        preferences.relay_url,
        preferences.background_enabled,
        preferences.background_image,
        preferences.background_opacity,
        preferences.background_blur,
        preferences.background_brightness,
        preferences.background_card_blur,
    );
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(content.as_bytes())
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    let persisted = temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    persisted.sync_all().map_err(|error| error.to_string())?;

    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn parse_preferences(content: &str) -> Preferences {
    let mut preferences = Preferences::default();
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("theme=") {
            let value = value.trim();
            if matches!(value, "system" | "light" | "dark") {
                preferences.theme = value.to_owned();
            }
        } else if let Some(value) = line.strip_prefix("color_theme=") {
            let value = value.trim();
            preferences.color_theme = normalize_color_theme(value).to_owned();
        } else if let Some(value) = line.strip_prefix("custom_theme=") {
            if let Ok(theme) = serde_json::from_str::<CustomTheme>(value.trim())
                && is_custom_theme(&theme)
            {
                preferences.custom_theme = theme;
            }
        } else if let Some(value) = line.strip_prefix("font_size=") {
            preferences.font_size = value
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|size| FONT_SIZE_RANGE.contains(size))
                .unwrap_or(14);
        } else if let Some(value) = line.strip_prefix("font_family=") {
            if let Some(font_family) = normalize_font_family(value) {
                preferences.font_family = font_family;
            }
        } else if let Some(value) = line.strip_prefix("splash_duration_ms=") {
            preferences.splash_duration_ms = value
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|duration| SPLASH_DURATION_OPTIONS_MS.contains(duration))
                .unwrap_or(1000);
        } else if let Some(value) = line.strip_prefix("silent_start=") {
            preferences.silent_start = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("auto_update=") {
            preferences.auto_update = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("locale=") {
            let value = value.trim();
            if matches!(value, "zh-CN" | "en") {
                preferences.locale = value.to_owned();
            }
        } else if let Some(value) = line.strip_prefix("remember_window_state=") {
            preferences.remember_window_state = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("window_material=") {
            let value = value.trim();
            if is_window_material(value) {
                preferences.window_material = value.to_owned();
            }
        } else if let Some(value) = line.strip_prefix("auto_lightweight_minutes=") {
            preferences.auto_lightweight_minutes = value
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|minutes| AUTO_LIGHTWEIGHT_MINUTES_RANGE.contains(minutes));
        } else if let Some(value) = line.strip_prefix("host_uri_lifetime=") {
            let value = value.trim();
            if is_invite_lifetime(value) {
                preferences.host_uri_lifetime = value.to_owned();
            }
        } else if let Some(value) = line.strip_prefix("join_uri=") {
            preferences.join_uri = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("join_port=")
            && let Ok(port) = value.trim().parse::<u16>()
            && port != 0
        {
            preferences.join_port = port;
        } else if let Some(value) = line.strip_prefix("reconnect_timeout_secs=") {
            let value = value.trim();
            preferences.reconnect_timeout_secs = value
                .parse::<u64>()
                .ok()
                .filter(|timeout| RECONNECT_TIMEOUT_OPTIONS_SECS.contains(timeout));
        } else if let Some(value) = line.strip_prefix("relay_custom=") {
            preferences.relay_custom = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("relay_url=") {
            preferences.relay_url = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("background_enabled=") {
            preferences.background_enabled = value.trim() == "true";
        } else if let Some(value) = line.strip_prefix("background_image=") {
            preferences.background_image = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("background_opacity=") {
            preferences.background_opacity = value
                .trim()
                .parse()
                .ok()
                .filter(|v: &f32| (0.0..=1.0).contains(v))
                .unwrap_or(0.75);
        } else if let Some(value) = line.strip_prefix("background_blur=") {
            preferences.background_blur = value
                .trim()
                .parse()
                .ok()
                .filter(|v: &u32| *v <= 20)
                .unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("background_brightness=") {
            preferences.background_brightness = value
                .trim()
                .parse()
                .ok()
                .filter(|v: &f32| (0.5..=1.5).contains(v))
                .unwrap_or(1.0);
        } else if let Some(value) = line.strip_prefix("background_card_blur=") {
            preferences.background_card_blur = value
                .trim()
                .parse()
                .ok()
                .filter(|v: &u32| (8..=30).contains(v))
                .unwrap_or(8);
        }
    }
    preferences
}

fn is_custom_theme(theme: &CustomTheme) -> bool {
    [&theme.light, &theme.dark].into_iter().all(|colors| {
        [
            &colors.bg,
            &colors.bg_secondary,
            &colors.bg_tertiary,
            &colors.primary,
            &colors.primary_solid,
            &colors.primary_solid_hover,
            &colors.secondary,
            &colors.text_primary,
            &colors.text_secondary,
            &colors.border,
        ]
        .into_iter()
        .all(|color| {
            color.len() == 7
                && color.starts_with('#')
                && color[1..]
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        })
    })
}

fn normalize_font_family(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() <= 128 && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

fn normalize_color_theme(value: &str) -> &'static str {
    match value {
        "default" => "default",
        "inkstone" | "neutral" | "midnight" => "inkstone",
        "vellum" | "warm" | "sunset" => "vellum",
        "moss" | "mountain" | "sage" | "ocean" => "moss",
        "gloaming" | "mauve" | "rose" => "gloaming",
        "custom" => "custom",
        _ => "default",
    }
}

fn is_invite_lifetime(value: &str) -> bool {
    matches!(
        value,
        "always" | "never" | "1h" | "3h" | "6h" | "12h" | "24h"
    )
}

fn is_window_material(value: &str) -> bool {
    matches!(
        value,
        "solid" | "mica" | "acrylic" | "vibrancy" | "liquid_glass"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_missing_values() {
        let preferences = parse_preferences("");

        assert_eq!(preferences, Preferences::default());
    }

    #[test]
    fn parses_preferences() {
        let preferences = parse_preferences(
            "theme=dark\ncolor_theme=moss\nfont_size=17\nfont_family=Microsoft YaHei\nsplash_duration_ms=2000\nsilent_start=true\nlocale=en\nremember_window_state=false\nwindow_material=acrylic\nauto_lightweight_minutes=10\njoin_uri=sculk://join/v1/example\njoin_port=25566\nreconnect_timeout_secs=30\nrelay_custom=true\nrelay_url=https://relay.example.com\nwindow_x=100\nwindow_y=200\nwindow_width=960\nwindow_height=640\nwindow_maximized=true\n",
        );

        assert_eq!(preferences.theme, "dark");
        assert_eq!(preferences.color_theme, "moss");
        assert_eq!(preferences.font_size, 17);
        assert_eq!(preferences.font_family, "Microsoft YaHei");
        assert_eq!(preferences.splash_duration_ms, 2000);
        assert!(preferences.silent_start);
        assert_eq!(preferences.locale, "en");
        assert!(!preferences.remember_window_state);
        assert_eq!(preferences.window_material, "acrylic");
        assert_eq!(preferences.auto_lightweight_minutes, Some(10));
        assert_eq!(preferences.join_uri, "sculk://join/v1/example");
        assert_eq!(preferences.join_port, 25_566);
        assert_eq!(preferences.reconnect_timeout_secs, Some(30));
        assert!(preferences.relay_custom);
        assert_eq!(preferences.relay_url, "https://relay.example.com");
    }

    #[test]
    fn falls_back_to_default_theme() {
        let preferences = parse_preferences(
            "theme=neon\ncolor_theme=unknown\nfont_size=30\nsplash_duration_ms=4000\nlocale=fr\nauto_lightweight_minutes=0\nreconnect_timeout_secs=45\n",
        );

        assert_eq!(preferences.theme, "system");
        assert_eq!(preferences.color_theme, "default");
        assert_eq!(preferences.font_size, 14);
        assert_eq!(preferences.splash_duration_ms, 1000);
        assert_eq!(preferences.locale, "zh-CN");
        assert_eq!(preferences.auto_lightweight_minutes, None);
        assert_eq!(preferences.reconnect_timeout_secs, None);
    }

    #[test]
    fn parses_liquid_glass_material() {
        let preferences = parse_preferences("window_material=liquid_glass\n");

        assert_eq!(preferences.window_material, "liquid_glass");
    }

    #[test]
    fn parses_custom_theme() {
        let mut custom_theme = CustomTheme::default();
        custom_theme.light.primary = "#123456".to_owned();
        let encoded = serde_json::to_string(&custom_theme).expect("custom theme should serialize");
        let preferences =
            parse_preferences(&format!("color_theme=custom\ncustom_theme={encoded}\n"));

        assert_eq!(preferences.color_theme, "custom");
        assert_eq!(preferences.custom_theme, custom_theme);
    }

    #[test]
    fn migrates_old_palette() {
        for (old, current) in [
            ("default", "default"),
            ("neutral", "inkstone"),
            ("midnight", "inkstone"),
            ("warm", "vellum"),
            ("sunset", "vellum"),
            ("mountain", "moss"),
            ("sage", "moss"),
            ("ocean", "moss"),
            ("mauve", "gloaming"),
            ("rose", "gloaming"),
        ] {
            assert_eq!(
                parse_preferences(&format!("color_theme={old}\n")).color_theme,
                current
            );
        }
    }

    #[test]
    fn preserves_invite_atomically() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join(PREFERENCES_FILE);
        std::fs::write(&path, "theme=light\n").expect("initial preferences should be written");

        let preferences = Preferences {
            theme: "dark".to_owned(),
            join_uri: "sculk://join/v1/remember-me".to_owned(),
            ..Preferences::default()
        };
        save_preferences(&path, &preferences).expect("preferences should be replaced");

        let saved = std::fs::read_to_string(&path).expect("preferences should remain readable");
        assert_eq!(parse_preferences(&saved), preferences);
        assert_eq!(
            std::fs::read_dir(directory.path())
                .expect("directory should be readable")
                .count(),
            1
        );
    }
}

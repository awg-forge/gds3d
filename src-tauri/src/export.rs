use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewCapture {
    data_url: String,
    width: u32,
    height: u32,
}

#[tauri::command]
pub async fn export_model(path: String, data_url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let encoded = data_url
            .split_once(',')
            .map(|(_, encoded)| encoded)
            .ok_or_else(|| "invalid model data".to_owned())?;
        let data = STANDARD
            .decode(encoded)
            .map_err(|error| error.to_string())?;
        fs::write(path, data).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn export_view(path: String, capture: ViewCapture) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || write_export(&path, capture))
        .await
        .map_err(|error| error.to_string())??;
    Ok(())
}

fn write_export(path: &str, capture: ViewCapture) -> Result<(), String> {
    if capture.width == 0 || capture.height == 0 {
        return Err("invalid viewport size".to_owned());
    }
    let encoded = capture
        .data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| "invalid PNG capture".to_owned())?;
    let png = STANDARD
        .decode(encoded)
        .map_err(|error| error.to_string())?;
    fs::write(path, png).map_err(|error| error.to_string())
}

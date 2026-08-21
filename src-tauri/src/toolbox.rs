use iroh::{Endpoint, RelayMap, RelayMode, Watcher as _, endpoint::presets};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkDiagnostics {
    pub public_ipv4: Option<String>,
    pub public_ipv6: Option<String>,
    pub udp_available: bool,
    pub mapping_varies_by_destination: Option<bool>,
    pub relay_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelayDiagnostics {
    pub relay_url: Option<String>,
    pub latency_ms: Option<u64>,
}

async fn make_endpoint(custom_relay: Option<String>) -> Result<Endpoint, String> {
    let endpoint = if let Some(url) = custom_relay {
        let relay_url: iroh::RelayUrl = url
            .parse()
            .map_err(|_| "toolbox_relay_invalid_url".to_owned())?;
        Endpoint::builder(presets::N0)
            .relay_mode(RelayMode::Custom(RelayMap::from(relay_url)))
            .bind()
            .await
    } else {
        Endpoint::bind(presets::N0).await
    }
    .map_err(|error| {
        log::warn!("toolbox network diagnostics could not start: {error}");
        "toolbox_network_start_failed".to_owned()
    })?;

    Ok(endpoint)
}

#[tauri::command]
pub(crate) async fn run_network_diagnostics() -> Result<NetworkDiagnostics, String> {
    let endpoint = make_endpoint(None).await?;
    let mut watcher = endpoint.net_report();
    let report = tokio::time::timeout(Duration::from_secs(12), watcher.initialized())
        .await
        .map_err(|_| "toolbox_network_timeout".to_owned())?;
    Ok(NetworkDiagnostics {
        public_ipv4: report.global_v4.map(|address| address.ip().to_string()),
        public_ipv6: report.global_v6.map(|address| address.ip().to_string()),
        udp_available: report.has_udp(),
        mapping_varies_by_destination: report.mapping_varies_by_dest(),
        relay_available: report.preferred_relay.is_some(),
    })
}

#[tauri::command]
pub(crate) async fn run_relay_diagnostics(
    relay_url: Option<String>,
) -> Result<RelayDiagnostics, String> {
    let endpoint = make_endpoint(relay_url).await?;
    let mut watcher = endpoint.net_report();
    let report = tokio::time::timeout(Duration::from_secs(12), watcher.initialized())
        .await
        .map_err(|_| "toolbox_network_timeout".to_owned())?;
    let relay_url = report.preferred_relay.as_ref().map(ToString::to_string);
    let latency_ms = report.preferred_relay.as_ref().and_then(|preferred| {
        report
            .relay_latency
            .iter()
            .find(|(_, url, _)| *url == preferred)
            .map(|(_, _, latency)| latency.as_millis() as u64)
    });
    Ok(RelayDiagnostics {
        relay_url,
        latency_ms,
    })
}

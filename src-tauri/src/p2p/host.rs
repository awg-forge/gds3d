use crate::p2p::{self, P2pMode, P2pPeerSnapshot, P2pSnapshot, P2pState};
use crate::settings::SettingsState;
use sculk::ErrorCategory;
use sculk::minecraft::lan::LanScanner;
use sculk::minecraft::probe_server;
use sculk::persist::{self, HostState as PersistedHostState};
use sculk::tunnel::{
    HostConfig, HostedServiceHandle, HostedServiceOptions, HostedServicePhase, HostedServiceStatus,
    NodeOptions, SculkNode, SecretKey, ServiceId, TokenRefreshPolicy, TunnelEvent,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tokio::sync::{broadcast, mpsc};

const HOST_START_TIMEOUT: Duration = Duration::from_secs(15);
const MC_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const MC_HEALTH_INTERVAL: Duration = Duration::from_secs(5);
const MC_HEALTH_FAILURES_MAX: u8 = 3;
const JOIN_URI_PREFIX: &str = "sculk://join/v1/";
const SHARE_URL_PREFIX: &str = "https://ideaflash.cn/#v1/";

pub(crate) struct HostState {
    scanner: Mutex<Option<LanScanner>>,
    detected_port: Mutex<Option<u16>>,
    task: Mutex<Option<HostTask>>,
    status: Mutex<Option<HostedServiceStatus>>,
    uri: Mutex<Option<String>>,
    port: Mutex<Option<u16>>,
    message: Mutex<Option<String>>,
    peers: Mutex<BTreeMap<String, P2pPeerSnapshot>>,
    generation: AtomicU64,
}

impl HostState {
    pub(crate) fn new() -> Self {
        Self {
            scanner: Mutex::new(None),
            detected_port: Mutex::new(None),
            task: Mutex::new(None),
            status: Mutex::new(None),
            uri: Mutex::new(None),
            port: Mutex::new(None),
            message: Mutex::new(None),
            peers: Mutex::new(BTreeMap::new()),
            generation: AtomicU64::new(0),
        }
    }

    fn is_running(&self) -> bool {
        self.task.lock().is_ok_and(|task| task.is_some())
    }

    fn snapshot(&self) -> P2pSnapshot {
        snapshot(
            self.status.lock().ok().and_then(|status| status.clone()),
            self.uri.lock().ok().and_then(|uri| uri.clone()),
            self.message.lock().ok().and_then(|message| message.clone()),
            self.port.lock().ok().and_then(|port| *port),
            self.peers(),
        )
    }

    fn set_message(&self, message: Option<String>) {
        if let Ok(mut current) = self.message.lock() {
            *current = message;
        }
    }

    fn peers(&self) -> Vec<P2pPeerSnapshot> {
        self.peers
            .lock()
            .map(|peers| peers.values().cloned().collect())
            .unwrap_or_default()
    }

    fn clear_peers(&self) {
        if let Ok(mut peers) = self.peers.lock() {
            peers.clear();
        }
    }

    fn apply_event(&self, event: &TunnelEvent) {
        let Ok(mut peers) = self.peers.lock() else {
            return;
        };
        match event {
            TunnelEvent::PlayerJoined { id } => {
                let id = id.to_string();
                peers.entry(id.clone()).or_insert(P2pPeerSnapshot {
                    id,
                    route: None,
                    rtt_ms: None,
                });
            }
            TunnelEvent::PlayerLeft { id, .. } => {
                peers.remove(&id.to_string());
            }
            TunnelEvent::PathChanged {
                remote_id,
                is_relay,
                rtt_ms,
            } => {
                let id = remote_id.to_string();
                let peer = peers.entry(id.clone()).or_insert(P2pPeerSnapshot {
                    id,
                    route: None,
                    rtt_ms: None,
                });
                peer.route = Some(if *is_relay { "relay" } else { "direct" });
                peer.rtt_ms = Some(*rtt_ms);
            }
            _ => {}
        }
    }

    fn take_scanner(&self) -> Result<Option<LanScanner>, String> {
        *self
            .detected_port
            .lock()
            .map_err(|_| "LAN scan state is unavailable".to_owned())? = None;
        self.scanner
            .lock()
            .map_err(|_| "LAN scan state is unavailable".to_owned())
            .map(|mut scanner| scanner.take())
    }

    async fn stop_scanner(&self) -> Result<(), String> {
        let Some(scanner) = self.take_scanner()? else {
            return Ok(());
        };
        tokio::task::spawn_blocking(move || scanner.stop())
            .await
            .map_err(|error| format!("LAN scan stop task failed: {error}"))?
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LanScanSnapshot {
    scanning: bool,
    port: Option<u16>,
}

#[tauri::command]
pub(crate) fn start_lan_scan(state: State<'_, HostState>) -> Result<LanScanSnapshot, String> {
    let detected = *state
        .detected_port
        .lock()
        .map_err(|_| "LAN scan state is unavailable".to_owned())?;
    let mut scanner = state
        .scanner
        .lock()
        .map_err(|_| "LAN scan state is unavailable".to_owned())?;
    if detected.is_none() && scanner.is_none() {
        *scanner = Some(LanScanner::start().map_err(|error| error.to_string())?);
    }
    Ok(LanScanSnapshot {
        scanning: scanner.is_some(),
        port: detected,
    })
}

#[tauri::command]
pub(crate) fn get_lan_scan(state: State<'_, HostState>) -> Result<LanScanSnapshot, String> {
    let mut scanner = state
        .scanner
        .lock()
        .map_err(|_| "LAN scan state is unavailable".to_owned())?;
    let mut detected = state
        .detected_port
        .lock()
        .map_err(|_| "LAN scan state is unavailable".to_owned())?;
    if let Some(current) = scanner.as_ref() {
        while let Ok(port) = current.try_recv() {
            *detected = Some(port.get());
        }
        if current.is_finished() {
            scanner.take();
        }
    }
    Ok(LanScanSnapshot {
        scanning: scanner.is_some(),
        port: *detected,
    })
}

#[tauri::command]
pub(crate) async fn restart_lan_scan(
    state: State<'_, HostState>,
) -> Result<LanScanSnapshot, String> {
    state.stop_scanner().await?;
    start_lan_scan(state)
}

#[tauri::command]
pub(crate) async fn stop_lan_scan(state: State<'_, HostState>) -> Result<(), String> {
    state.stop_scanner().await
}

#[tauri::command]
pub(crate) async fn probe_host_port(port: u16) -> bool {
    port != 0 && minecraft_available(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await
}

#[tauri::command]
pub(crate) async fn start_host(
    port: u16,
    max_players: Option<u32>,
    uri_lifetime: String,
    app: AppHandle,
    p2p_state: State<'_, P2pState>,
    host_state: State<'_, HostState>,
) -> Result<(), String> {
    if host_state.is_running() {
        return Err("stop the current P2P session first".to_owned());
    }
    if port == 0 {
        return Err("Minecraft port must be between 1 and 65535".to_owned());
    }
    let target = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    if !minecraft_available(target).await {
        return Err(format!(
            "no Minecraft world is available on port {port}; make sure the world is open to LAN"
        ));
    }
    let token_refresh = token_refresh_policy(&uri_lifetime)
        .ok_or_else(|| "invalid room invitation lifetime".to_owned())?;
    let settings = app.state::<SettingsState>();
    let start = HostStart {
        mc_port: port,
        max_players,
        secret_key: settings.host_secret_key(),
        relay_url: settings.relay_url()?,
        token_refresh,
        state_path: settings.host_state_path(),
    };
    host_state.stop_scanner().await?;
    p2p_state.acquire(P2pMode::Host)?;

    host_state.set_message(None);
    host_state.clear_peers();
    *host_state
        .port
        .lock()
        .map_err(|_| "host state is unavailable".to_owned())? = Some(port);
    *host_state
        .status
        .lock()
        .map_err(|_| "host state is unavailable".to_owned())? = None;
    *host_state
        .uri
        .lock()
        .map_err(|_| "host state is unavailable".to_owned())? = None;
    let generation = host_state
        .generation
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    let task = HostTask::spawn(start, spawn_update_loop(&app, generation));
    match host_state.task.lock() {
        Ok(mut current) => *current = Some(task),
        Err(_) => {
            p2p_state.release(P2pMode::Host);
            return Err("host state is unavailable".to_owned());
        }
    }
    p2p_state.publish(&app, host_state.snapshot());
    Ok(())
}

pub(crate) fn stop(app: &AppHandle) -> Result<(), String> {
    let host_state = app.state::<HostState>();
    let requested = host_state
        .task
        .lock()
        .map_err(|_| "host state is unavailable".to_owned())?
        .as_ref()
        .is_some_and(HostTask::stop);
    if !requested {
        return Err("room stop task is unavailable".to_owned());
    }
    if let Ok(mut status) = host_state.status.lock()
        && let Some(status) = status.as_mut()
    {
        status.phase = HostedServicePhase::Stopping;
    }
    app.state::<P2pState>().publish(app, host_state.snapshot());
    Ok(())
}

pub struct HostStart {
    pub mc_port: u16,
    pub max_players: Option<u32>,
    pub secret_key: SecretKey,
    pub relay_url: Option<sculk::tunnel::RelayUrl>,
    pub token_refresh: TokenRefreshPolicy,
    pub state_path: PathBuf,
}

pub enum HostUpdate {
    Started {
        uri: String,
        status: HostedServiceStatus,
    },
    Status(HostedServiceStatus),
    UriChanged {
        uri: String,
        status: HostedServiceStatus,
    },
    Event(TunnelEvent),
    Error(String),
    MinecraftUnavailable,
    Failed(String),
    Stopped(Result<(), String>),
}

enum HostCommand {
    Stop,
}

pub struct HostTask {
    commands: mpsc::UnboundedSender<HostCommand>,
}

impl HostTask {
    pub fn spawn(start: HostStart, updates: mpsc::UnboundedSender<HostUpdate>) -> Self {
        let (commands, command_rx) = mpsc::unbounded_channel();
        tauri::async_runtime::spawn(run_host(start, command_rx, updates));
        Self { commands }
    }

    pub fn stop(&self) -> bool {
        self.commands.send(HostCommand::Stop).is_ok()
    }
}

pub fn token_refresh_policy(value: &str) -> Option<TokenRefreshPolicy> {
    match value {
        "always" => Some(TokenRefreshPolicy::Always),
        "never" => Some(TokenRefreshPolicy::Never),
        "1h" => Some(TokenRefreshPolicy::After(Duration::from_secs(60 * 60))),
        "3h" => Some(TokenRefreshPolicy::After(Duration::from_secs(3 * 60 * 60))),
        "6h" => Some(TokenRefreshPolicy::After(Duration::from_secs(6 * 60 * 60))),
        "12h" => Some(TokenRefreshPolicy::After(Duration::from_secs(12 * 60 * 60))),
        "24h" => Some(TokenRefreshPolicy::After(Duration::from_secs(24 * 60 * 60))),
        _ => None,
    }
}

fn spawn_update_loop(app: &AppHandle, generation: u64) -> mpsc::UnboundedSender<HostUpdate> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(update) = rx.recv().await {
            let host_state = app.state::<HostState>();
            if host_state.generation.load(Ordering::Relaxed) != generation {
                return;
            }
            apply_host_update(&app, &host_state, update);
        }
    });
    tx
}

fn apply_host_update(app: &AppHandle, state: &HostState, update: HostUpdate) {
    let terminal = match update {
        HostUpdate::Started { uri, status } | HostUpdate::UriChanged { uri, status } => {
            if let Ok(mut current) = state.uri.lock() {
                *current = Some(uri);
            }
            if let Ok(mut current) = state.status.lock() {
                *current = Some(status);
            }
            state.set_message(None);
            None
        }
        HostUpdate::Status(status) => {
            if let Ok(mut current) = state.status.lock() {
                *current = Some(status);
            }
            None
        }
        HostUpdate::Event(event) => {
            state.apply_event(&event);
            state.set_message(p2p::event_message(event));
            None
        }
        HostUpdate::Error(error) => {
            state.set_message(Some(error));
            None
        }
        HostUpdate::MinecraftUnavailable => Some(Some(
            "the Minecraft world was closed, so the room stopped automatically".to_owned(),
        )),
        HostUpdate::Failed(error) => Some(Some(error)),
        HostUpdate::Stopped(result) => Some(result.err()),
    };

    let p2p_state = app.state::<P2pState>();
    if let Some(message) = terminal {
        clear_session(state);
        p2p_state.release(P2pMode::Host);
        p2p_state.publish(app, P2pSnapshot::idle(message));
    } else if p2p_state.active_mode() == Some(P2pMode::Host) {
        p2p_state.publish(app, state.snapshot());
    }
}

fn clear_session(state: &HostState) {
    if let Ok(mut task) = state.task.lock() {
        *task = None;
    }
    if let Ok(mut status) = state.status.lock() {
        *status = None;
    }
    if let Ok(mut uri) = state.uri.lock() {
        *uri = None;
    }
    if let Ok(mut port) = state.port.lock() {
        *port = None;
    }
    state.clear_peers();
}

fn to_share_url(uri: &str) -> Option<String> {
    let payload = uri.strip_prefix(JOIN_URI_PREFIX)?;
    (!payload.is_empty()).then(|| format!("{SHARE_URL_PREFIX}{payload}"))
}

fn snapshot(
    status: Option<HostedServiceStatus>,
    share_uri: Option<String>,
    message: Option<String>,
    host_port: Option<u16>,
    host_peers: Vec<P2pPeerSnapshot>,
) -> P2pSnapshot {
    P2pSnapshot {
        phase: match status.as_ref().map(|status| status.phase) {
            None => "starting",
            Some(HostedServicePhase::Active) => "active",
            Some(HostedServicePhase::Stopping) => "stopping",
            Some(HostedServicePhase::Stopped) => "idle",
        },
        mode: Some("host"),
        local_address: None,
        share_uri: share_uri.and_then(|uri| to_share_url(&uri)),
        player_count: status.as_ref().map_or(0, |status| status.connection_count),
        host_port,
        route: None,
        rtt_ms: None,
        tx_bytes: 0,
        rx_bytes: 0,
        host_peers,
        error: status
            .and_then(|status| status.last_error)
            .map(p2p::category_name),
        message,
    }
}

async fn run_host(
    start: HostStart,
    mut commands: mpsc::UnboundedReceiver<HostCommand>,
    updates: mpsc::UnboundedSender<HostUpdate>,
) {
    let target_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, start.mc_port));
    let saved = match persist::load_host_state(&start.state_path) {
        Ok(saved) => saved,
        Err(error) => return send(&updates, HostUpdate::Failed(error.to_string())),
    };
    let service_id = saved
        .as_ref()
        .map_or_else(ServiceId::generate, |state| state.service_id);
    let token_state = saved.map(|state| state.token_state);
    let node = match tokio::time::timeout(
        HOST_START_TIMEOUT,
        SculkNode::bind(NodeOptions {
            secret_key: Some(start.secret_key),
            relay_url: start.relay_url,
            ..NodeOptions::default()
        }),
    )
    .await
    {
        Ok(Ok(node)) => node,
        Ok(Err(error)) => return send(&updates, HostUpdate::Failed(error.to_string())),
        Err(_) => {
            return send(
                &updates,
                HostUpdate::Failed("node startup timed out; check relay settings".to_owned()),
            );
        }
    };
    let host = match node
        .start_service(HostedServiceOptions {
            service_id,
            target_addr,
            token_state,
            token_refresh: start.token_refresh,
            config: HostConfig::new()
                .event_delay(Duration::from_secs(1))
                .max_players(start.max_players),
        })
        .await
    {
        Ok(host) => host,
        Err(error) => {
            node.close().await;
            return send(&updates, HostUpdate::Failed(error.to_string()));
        }
    };
    let mut events = match host.subscribe().await {
        Ok(events) => events,
        Err(error) => {
            node.close().await;
            return send(&updates, HostUpdate::Failed(error.to_string()));
        }
    };
    let mut statuses = match host.subscribe_status().await {
        Ok(statuses) => statuses,
        Err(error) => {
            node.close().await;
            return send(&updates, HostUpdate::Failed(error.to_string()));
        }
    };
    if let Err(error) = save_state(&start.state_path, &host).await {
        node.close().await;
        return send(&updates, HostUpdate::Failed(error));
    }
    let status = match host.status().await {
        Ok(status) => status,
        Err(error) => {
            node.close().await;
            return send(&updates, HostUpdate::Failed(error.to_string()));
        }
    };
    let uri = match read_join_uri(&host).await {
        Ok(uri) => uri,
        Err(error) => {
            node.close().await;
            return send(&updates, HostUpdate::Failed(error));
        }
    };
    let mut uri_generation = status.uri_generation;
    if updates.send(HostUpdate::Started { uri, status }).is_err() {
        node.close().await;
        return;
    }

    let first_health_check = tokio::time::Instant::now() + MC_HEALTH_INTERVAL;
    let mut health_checks = tokio::time::interval_at(first_health_check, MC_HEALTH_INTERVAL);
    health_checks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut health_failures = 0_u8;
    let mut pending_target_error = None;
    loop {
        tokio::select! {
            command = commands.recv() => {
                if matches!(command, Some(HostCommand::Stop)) {
                    let result = host.stop().await.map_err(|error| error.to_string());
                    node.close().await;
                    send(&updates, HostUpdate::Stopped(result));
                } else {
                    node.close().await;
                }
                return;
            }
            event = events.recv() => {
                match event {
                    Ok(event) if is_target_error(&event) => {
                        pending_target_error.get_or_insert(event);
                    }
                    Ok(event) => send(&updates, HostUpdate::Event(event)),
                    Err(broadcast::error::RecvError::Lagged(count)) => send(
                        &updates,
                        HostUpdate::Error(format!("missed {count} host events")),
                    ),
                    Err(broadcast::error::RecvError::Closed) => {
                        node.close().await;
                        send(&updates, HostUpdate::Failed(
                            "host event channel closed unexpectedly".to_owned(),
                        ));
                        return;
                    }
                }
            }
            status = statuses.recv() => {
                let Some(status) = status else {
                    node.close().await;
                    send(&updates, HostUpdate::Failed(
                        "host status channel closed unexpectedly".to_owned(),
                    ));
                    return;
                };
                if status.uri_generation > uri_generation {
                    uri_generation = status.uri_generation;
                    if let Err(error) = save_state(&start.state_path, &host).await {
                        node.close().await;
                        send(&updates, HostUpdate::Failed(error));
                        return;
                    }
                    match read_join_uri(&host).await {
                        Ok(uri) => send(&updates, HostUpdate::UriChanged { uri, status }),
                        Err(error) => {
                            node.close().await;
                            send(&updates, HostUpdate::Failed(error));
                            return;
                        }
                    }
                } else {
                    send(&updates, HostUpdate::Status(status));
                }
            }
            _ = health_checks.tick() => {
                let available = minecraft_available(target_addr).await;
                if available && let Some(event) = pending_target_error.take() {
                    send(&updates, HostUpdate::Event(event));
                }
                if !record_health_check(&mut health_failures, available) {
                    continue;
                }
                let result = host.stop().await.map_err(|error| error.to_string());
                node.close().await;
                send(&updates, match result {
                    Ok(()) => HostUpdate::MinecraftUnavailable,
                    Err(error) => HostUpdate::Failed(error),
                });
                return;
            }
        }
    }
}

fn send(updates: &mpsc::UnboundedSender<HostUpdate>, update: HostUpdate) {
    let _ = updates.send(update);
}

async fn minecraft_available(addr: SocketAddr) -> bool {
    tokio::task::spawn_blocking(move || probe_server(addr, MC_PROBE_TIMEOUT))
        .await
        .is_ok_and(|result| result.is_ok())
}

fn record_health_check(failures: &mut u8, available: bool) -> bool {
    if available {
        *failures = 0;
        return false;
    }
    *failures = failures.saturating_add(1);
    *failures >= MC_HEALTH_FAILURES_MAX
}

fn is_target_error(event: &TunnelEvent) -> bool {
    matches!(
        event,
        TunnelEvent::Error {
            category: ErrorCategory::TargetUnavailable,
            ..
        }
    )
}

async fn save_state(path: &Path, host: &HostedServiceHandle) -> Result<(), String> {
    let token_state = host
        .token_state()
        .await
        .map_err(|error| error.to_string())?;
    persist::save_host_state(
        path,
        &PersistedHostState {
            service_id: host.service_id(),
            token_state,
        },
    )
    .map_err(|error| error.to_string())
}

async fn read_join_uri(host: &HostedServiceHandle) -> Result<String, String> {
    host.join_uri()
        .await
        .map_err(|error| error.to_string())?
        .expose_secret_uri()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_timed_refresh() {
        assert_eq!(
            token_refresh_policy("3h"),
            Some(TokenRefreshPolicy::After(Duration::from_secs(3 * 60 * 60)))
        );
        assert_eq!(token_refresh_policy("invalid"), None);
    }

    #[test]
    fn maps_refresh_policies() {
        assert_eq!(
            token_refresh_policy("always"),
            Some(TokenRefreshPolicy::Always)
        );
        assert_eq!(
            token_refresh_policy("never"),
            Some(TokenRefreshPolicy::Never)
        );
        for (value, hours) in [("1h", 1), ("3h", 3), ("6h", 6), ("12h", 12), ("24h", 24)] {
            assert_eq!(
                token_refresh_policy(value),
                Some(TokenRefreshPolicy::After(Duration::from_secs(
                    hours * 60 * 60
                )))
            );
        }
    }

    #[test]
    fn counts_health_failures() {
        let mut failures = 0;
        assert!(!record_health_check(&mut failures, false));
        assert!(!record_health_check(&mut failures, false));
        assert!(record_health_check(&mut failures, false));

        assert!(!record_health_check(&mut failures, true));
        assert_eq!(failures, 0);
    }

    #[test]
    fn starts_without_status() {
        let snapshot = snapshot(
            None,
            Some("sculk://join/v1/example".to_owned()),
            None,
            Some(25_565),
            Vec::new(),
        );

        assert_eq!(snapshot.phase, "starting");
        assert_eq!(snapshot.mode, Some("host"));
        assert_eq!(snapshot.host_port, Some(25_565));
        assert_eq!(
            snapshot.share_uri.as_deref(),
            Some("https://ideaflash.cn/#v1/example")
        );
        assert_eq!(snapshot.player_count, 0);
    }

    #[test]
    fn wraps_share_url() {
        assert_eq!(
            to_share_url("sculk://join/v1/payload_123-abc").as_deref(),
            Some("https://ideaflash.cn/#v1/payload_123-abc")
        );
        assert_eq!(to_share_url("sculk://join/v1/"), None);
        assert_eq!(to_share_url("https://example.com/invite"), None);
    }
}

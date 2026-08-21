use crate::p2p::{self, P2pMode, P2pSnapshot, P2pState};
use crate::settings::SettingsState;
use sculk::minecraft::lan::LanBroadcaster;
use sculk::tunnel::{
    JoinConfig, JoinOptions, JoinUri, LocalPort, TunnelMode, TunnelPhase, TunnelStatus,
    TunnelUpdate,
};
use serde::Serialize;
use std::num::NonZeroU16;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{App, AppHandle, Manager, State};

const LAN_NAME: &str = "SeaLantern Connect";

pub(crate) struct JoinState {
    broadcaster: Mutex<Option<LanBroadcaster>>,
    pending_uri: Mutex<Option<String>>,
    message: Mutex<Option<String>>,
}

impl JoinState {
    pub(crate) fn new() -> Self {
        Self {
            broadcaster: Mutex::new(None),
            pending_uri: Mutex::new(None),
            message: Mutex::new(None),
        }
    }

    fn set_message(&self, message: Option<String>) {
        if let Ok(mut current) = self.message.lock() {
            *current = message;
        }
    }

    fn message(&self) -> Option<String> {
        self.message.lock().ok().and_then(|message| message.clone())
    }

    fn set_pending_uri(&self, uri: Option<String>) {
        if let Ok(mut pending) = self.pending_uri.lock() {
            *pending = uri;
        }
    }

    fn take_pending_uri(&self) -> Option<String> {
        self.pending_uri
            .lock()
            .ok()
            .and_then(|mut pending| pending.take())
    }

    fn sync_broadcast(&self, status: &TunnelStatus) -> Result<(), String> {
        let mut current = self
            .broadcaster
            .lock()
            .map_err(|_| "LAN broadcast state is unavailable".to_owned())?;
        let desired_port = if status.state.phase == TunnelPhase::Active
            && status.state.mode == Some(TunnelMode::Join)
        {
            status
                .state
                .local_addr
                .and_then(|addr| NonZeroU16::new(addr.port()))
        } else {
            None
        };

        if desired_port.is_none() {
            if let Some(broadcaster) = current.take() {
                broadcaster.stop().map_err(|error| error.to_string())?;
            }
            return Ok(());
        }
        if current.as_ref().is_some_and(|item| !item.is_finished()) {
            return Ok(());
        }
        if let Some(broadcaster) = current.take() {
            let _ = broadcaster.stop();
        }
        *current = Some(
            LanBroadcaster::start(LAN_NAME, desired_port.expect("checked above"))
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    fn stop_broadcast(&self) {
        if let Ok(mut current) = self.broadcaster.lock()
            && let Some(broadcaster) = current.take()
        {
            let _ = broadcaster.stop();
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InvitePreview {
    valid: bool,
}

#[tauri::command]
pub(crate) fn validate_invite(uri: String) -> Result<InvitePreview, String> {
    uri.trim()
        .parse::<JoinUri>()
        .map(|_| InvitePreview { valid: true })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn start_join(
    uri: String,
    local_port: Option<u16>,
    app: AppHandle,
    settings: State<'_, SettingsState>,
    p2p_state: State<'_, P2pState>,
    join_state: State<'_, JoinState>,
) -> Result<(), String> {
    let uri = uri.trim().to_owned();
    let join_uri = uri.parse::<JoinUri>().map_err(|error| error.to_string())?;
    let config = JoinConfig::new()
        .event_delay(Duration::from_secs(1))
        .reconnect_timeout(settings.reconnect_timeout()?);
    let local_port = match local_port {
        Some(port) => LocalPort::Fixed(
            NonZeroU16::new(port)
                .ok_or_else(|| "local port must be between 1 and 65535".to_owned())?,
        ),
        None => LocalPort::Auto,
    };

    p2p_state.acquire(P2pMode::Join)?;
    join_state.stop_broadcast();
    join_state.set_message(None);
    join_state.set_pending_uri(Some(uri));
    if let Err(error) = p2p_state
        .tunnel()
        .start_join(
            JoinOptions::new(join_uri)
                .local_port(local_port)
                .config(config),
        )
        .await
    {
        join_state.set_pending_uri(None);
        p2p_state.release(P2pMode::Join);
        p2p_state.publish(&app, P2pSnapshot::idle(None));
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn stop_join(app: AppHandle) -> Result<(), String> {
    stop(&app).await
}

pub(crate) async fn stop(app: &AppHandle) -> Result<(), String> {
    let join_state = app.state::<JoinState>();
    join_state.stop_broadcast();
    join_state.set_message(None);
    join_state.set_pending_uri(None);
    app.state::<P2pState>()
        .tunnel()
        .shutdown()
        .await
        .map_err(|error| error.to_string())
}

pub(crate) fn setup(app: &mut App) {
    let handle = app.handle().clone();
    let service = app.state::<P2pState>().tunnel();
    tauri::async_runtime::spawn(async move {
        let mut updates = service.subscribe();
        while let Some(update) = updates.recv().await {
            apply_update(&handle, update);
        }
    });
}

fn apply_update(app: &AppHandle, update: TunnelUpdate) {
    let p2p_state = app.state::<P2pState>();
    if p2p_state.active_mode() != Some(P2pMode::Join) {
        return;
    }
    let join_state = app.state::<JoinState>();
    let status = match update {
        TunnelUpdate::Status(status) => {
            if let Err(error) = join_state.sync_broadcast(&status) {
                join_state.set_message(Some(format!("LAN broadcast failed: {error}")));
            }
            if status.state.phase == TunnelPhase::Active
                && let Some(uri) = join_state.take_pending_uri()
                && let Err(error) = app.state::<SettingsState>().remember_join_uri(uri)
            {
                join_state.set_message(Some(format!("failed to save preferences: {error}")));
            }
            status
        }
        TunnelUpdate::Event(event) => {
            join_state.set_message(p2p::event_message(event));
            p2p_state.tunnel().status()
        }
        _ => p2p_state.tunnel().status(),
    };

    if status.state.phase == TunnelPhase::Idle {
        join_state.stop_broadcast();
        join_state.set_pending_uri(None);
        p2p_state.release(P2pMode::Join);
    }
    p2p_state.publish(app, snapshot(status, join_state.message()));
}

fn snapshot(status: TunnelStatus, message: Option<String>) -> P2pSnapshot {
    let peer = status.connections.iter().find(|peer| peer.alive);
    P2pSnapshot {
        phase: match status.state.phase {
            TunnelPhase::Idle => "idle",
            TunnelPhase::Starting => "starting",
            TunnelPhase::Active => "active",
            TunnelPhase::Stopping => "stopping",
        },
        mode: status.state.mode.map(|mode| match mode {
            TunnelMode::Host => "host",
            TunnelMode::Join => "join",
        }),
        local_address: status.state.local_addr.map(|addr| addr.to_string()),
        share_uri: status
            .state
            .join_uri
            .as_ref()
            .and_then(|uri| uri.expose_secret_uri().ok()),
        player_count: status.connections.iter().filter(|peer| peer.alive).count(),
        host_port: None,
        route: peer.map(|value| if value.is_relay { "relay" } else { "direct" }),
        rtt_ms: peer.map(|value| value.rtt_ms),
        tx_bytes: peer.map_or(0, |value| value.tx_bytes),
        rx_bytes: peer.map_or(0, |value| value.rx_bytes),
        host_peers: Vec::new(),
        error: status.last_error.map(p2p::category_name),
        message,
    }
}

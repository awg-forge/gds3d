pub(crate) mod host;
pub(crate) mod join;

use sculk::ErrorCategory;
use sculk::tunnel::{TunnelEvent, TunnelService};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

const STATUS_EVENT: &str = "p2p-status";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum P2pMode {
    Host,
    Join,
}

pub(crate) struct P2pState {
    service: TunnelService,
    active_mode: Mutex<Option<P2pMode>>,
    snapshot: Mutex<P2pSnapshot>,
}

impl P2pState {
    pub(crate) fn new() -> Self {
        Self {
            service: TunnelService::new(),
            active_mode: Mutex::new(None),
            snapshot: Mutex::new(P2pSnapshot::idle(None)),
        }
    }

    pub(crate) fn tunnel(&self) -> TunnelService {
        self.service.clone()
    }

    pub(crate) fn acquire(&self, mode: P2pMode) -> Result<(), String> {
        let mut active = self
            .active_mode
            .lock()
            .map_err(|_| "P2P mode state is unavailable".to_owned())?;
        if active.is_some() {
            return Err("stop the current P2P session first".to_owned());
        }
        *active = Some(mode);
        Ok(())
    }

    pub(crate) fn release(&self, mode: P2pMode) {
        if let Ok(mut active) = self.active_mode.lock()
            && *active == Some(mode)
        {
            *active = None;
        }
    }

    pub(crate) fn active_mode(&self) -> Option<P2pMode> {
        self.active_mode.lock().ok().and_then(|active| *active)
    }

    pub(crate) fn publish(&self, app: &AppHandle, snapshot: P2pSnapshot) {
        if let Ok(mut current) = self.snapshot.lock() {
            *current = snapshot.clone();
        }
        let _ = app.emit(STATUS_EVENT, snapshot);
    }

    fn snapshot(&self) -> P2pSnapshot {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_else(|_| P2pSnapshot::idle(Some("P2P state is unavailable".into())))
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct P2pSnapshot {
    pub(crate) phase: &'static str,
    pub(crate) mode: Option<&'static str>,
    pub(crate) local_address: Option<String>,
    pub(crate) share_uri: Option<String>,
    pub(crate) player_count: usize,
    pub(crate) host_port: Option<u16>,
    pub(crate) route: Option<&'static str>,
    pub(crate) rtt_ms: Option<u64>,
    pub(crate) tx_bytes: u64,
    pub(crate) rx_bytes: u64,
    pub(crate) host_peers: Vec<P2pPeerSnapshot>,
    pub(crate) error: Option<&'static str>,
    pub(crate) message: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct P2pPeerSnapshot {
    pub(crate) id: String,
    pub(crate) route: Option<&'static str>,
    pub(crate) rtt_ms: Option<u64>,
}

impl P2pSnapshot {
    pub(crate) fn idle(message: Option<String>) -> Self {
        Self {
            phase: "idle",
            mode: None,
            local_address: None,
            share_uri: None,
            player_count: 0,
            host_port: None,
            route: None,
            rtt_ms: None,
            tx_bytes: 0,
            rx_bytes: 0,
            host_peers: Vec::new(),
            error: None,
            message,
        }
    }
}

#[tauri::command]
pub(crate) fn get_p2p_status(state: State<'_, P2pState>) -> P2pSnapshot {
    state.snapshot()
}

pub(crate) fn event_message(event: TunnelEvent) -> Option<String> {
    match event {
        TunnelEvent::Connected => Some("P2P link established".to_owned()),
        TunnelEvent::PlayerJoined { id } => Some(format!("player {id} joined")),
        TunnelEvent::PlayerLeft { id, .. } => Some(format!("player {id} left")),
        TunnelEvent::Disconnected { reason } => Some(format!("P2P link closed: {reason}")),
        TunnelEvent::Reconnecting { attempt } => {
            Some(format!("restoring P2P link, attempt {attempt}"))
        }
        TunnelEvent::Reconnected => Some("P2P link restored".to_owned()),
        TunnelEvent::PathChanged {
            is_relay, rtt_ms, ..
        } => Some(format!(
            "using a {} P2P path with {rtt_ms} ms latency",
            if is_relay { "relay" } else { "direct" }
        )),
        TunnelEvent::Error { message, .. } => Some(message),
        _ => None,
    }
}

pub(crate) fn category_name(category: ErrorCategory) -> &'static str {
    match category {
        ErrorCategory::InvalidJoinUri => "invalid_join_uri",
        ErrorCategory::InvalidEndpoint => "invalid_endpoint",
        ErrorCategory::AuthorizationDenied => "authorization_denied",
        ErrorCategory::HostUnreachable => "host_unreachable",
        ErrorCategory::TargetUnavailable => "target_unavailable",
        ErrorCategory::LocalPortUnavailable => "local_port_unavailable",
        ErrorCategory::IdentityUnavailable => "identity_unavailable",
        ErrorCategory::OperationConflict => "operation_conflict",
        ErrorCategory::ResourceLimit => "resource_limit",
        ErrorCategory::InvalidConfiguration => "invalid_configuration",
        ErrorCategory::Internal => "internal",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_event_messages() {
        assert_eq!(
            event_message(TunnelEvent::Connected),
            Some("P2P link established".to_owned())
        );
        assert_eq!(
            event_message(TunnelEvent::Reconnecting { attempt: 3 }),
            Some("restoring P2P link, attempt 3".to_owned())
        );
        assert_eq!(
            event_message(TunnelEvent::Reconnected),
            Some("P2P link restored".to_owned())
        );
    }

    #[test]
    fn keeps_error_names() {
        assert_eq!(
            category_name(ErrorCategory::AuthorizationDenied),
            "authorization_denied"
        );
        assert_eq!(
            category_name(ErrorCategory::LocalPortUnavailable),
            "local_port_unavailable"
        );
        assert_eq!(category_name(ErrorCategory::Internal), "internal");
    }

    #[test]
    fn keeps_modes_exclusive() {
        let state = P2pState::new();
        assert!(state.acquire(P2pMode::Host).is_ok());
        assert!(state.acquire(P2pMode::Join).is_err());
        state.release(P2pMode::Join);
        assert_eq!(state.active_mode(), Some(P2pMode::Host));
        state.release(P2pMode::Host);
        assert!(state.acquire(P2pMode::Join).is_ok());
    }
}

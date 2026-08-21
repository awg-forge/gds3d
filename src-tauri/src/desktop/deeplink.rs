use std::sync::Mutex;
use tauri::{App, AppHandle, Manager, State};

const JOIN_URI_PREFIX: &str = "sculk://join/v1/";
const MAX_JOIN_URI_LENGTH: usize = 512;
const MAX_PENDING_LINKS: usize = 8;

#[derive(Default)]
pub(crate) struct PendingDeepLinks {
    urls: Mutex<Vec<String>>,
}

fn is_join_uri(value: &str) -> bool {
    let Some(payload) = value.strip_prefix(JOIN_URI_PREFIX) else {
        return false;
    };
    !payload.is_empty()
        && value.len() <= MAX_JOIN_URI_LENGTH
        && payload
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(crate) fn stash_restore_links(app: &AppHandle, args: &[String]) {
    let urls: Vec<String> = args
        .iter()
        .filter(|url| is_join_uri(url))
        .cloned()
        .collect();
    if urls.is_empty() {
        return;
    }
    let state = app.state::<PendingDeepLinks>();
    if let Ok(mut pending) = state.urls.lock() {
        for url in &urls {
            if pending.len() >= MAX_PENDING_LINKS {
                break;
            }
            if !pending.contains(url) {
                pending.push(url.clone());
            }
        }
    }
}

#[tauri::command]
pub(crate) fn take_pending_links(state: State<'_, PendingDeepLinks>) -> Vec<String> {
    state
        .urls
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

pub fn setup(_app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(target_os = "linux", all(debug_assertions, target_os = "windows")))]
    {
        use tauri_plugin_deep_link::DeepLinkExt;

        _app.deep_link().register_all()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bounded_uris() {
        assert!(is_join_uri("sculk://join/v1/payload_123-abc"));
        assert!(!is_join_uri("sculk://join/v1/"));
        assert!(!is_join_uri("sculk://join/v2/payload"));
        assert!(!is_join_uri("sculk://join/v1/payload?query=1"));
        assert!(!is_join_uri(&format!(
            "sculk://join/v1/{}",
            "a".repeat(MAX_JOIN_URI_LENGTH)
        )));
    }
}

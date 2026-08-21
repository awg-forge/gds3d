use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard};
use tauri::menu::CheckMenuItem;
use tauri::{AppHandle, Manager, Wry};

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum MainWindowMode {
    Visible = 0,
    Hidden = 1,
    Background = 2,
}

impl MainWindowMode {
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Visible,
            2 => Self::Background,
            _ => Self::Hidden,
        }
    }

    fn allows(self, next: Self) -> bool {
        self == next
            || matches!(
                (self, next),
                (Self::Visible, Self::Hidden | Self::Background)
                    | (Self::Hidden, Self::Visible | Self::Background)
                    | (Self::Background, Self::Hidden)
            )
    }
}

pub(crate) struct MainWindowState {
    mode: AtomicU8,
    transition: Mutex<()>,
    tray_item: Mutex<Option<CheckMenuItem<Wry>>>,
}

impl MainWindowState {
    pub(crate) fn new() -> Self {
        Self {
            // tauri.conf.json creates the main window with visible: false.
            mode: AtomicU8::new(MainWindowMode::Hidden as u8),
            transition: Mutex::new(()),
            tray_item: Mutex::new(None),
        }
    }

    pub(crate) fn mode(&self) -> MainWindowMode {
        MainWindowMode::from_u8(self.mode.load(Ordering::Acquire))
    }

    pub(crate) fn is_background(&self) -> bool {
        self.mode() == MainWindowMode::Background
    }

    pub(crate) fn begin_transition(&self) -> Result<MainWindowTransition<'_>, String> {
        let guard = self
            .transition
            .lock()
            .map_err(|_| "main window transition lock is poisoned".to_owned())?;
        Ok(MainWindowTransition {
            state: self,
            _guard: guard,
        })
    }

    pub(crate) fn set_tray_item(&self, item: CheckMenuItem<Wry>) {
        let _ = item.set_checked(self.is_background());
        if let Ok(mut tray_item) = self.tray_item.lock() {
            *tray_item = Some(item);
        }
    }

    fn set_mode(&self, mode: MainWindowMode) {
        self.mode.store(mode as u8, Ordering::Release);
        if let Ok(tray_item) = self.tray_item.lock()
            && let Some(tray_item) = tray_item.as_ref()
        {
            let _ = tray_item.set_checked(mode == MainWindowMode::Background);
        }
    }
}

pub(crate) struct MainWindowTransition<'a> {
    state: &'a MainWindowState,
    _guard: MutexGuard<'a, ()>,
}

impl MainWindowTransition<'_> {
    pub(crate) fn mode(&self) -> MainWindowMode {
        self.state.mode()
    }

    pub(crate) fn move_to(&mut self, next: MainWindowMode) -> Result<(), String> {
        let current = self.mode();
        if current == next {
            return Ok(());
        }
        if !current.allows(next) {
            log::warn!("state rejected: {current:?} -> {next:?}");
            return Err(format!(
                "invalid main window transition: {current:?} -> {next:?}"
            ));
        }
        self.state.set_mode(next);
        log::debug!("window: {current:?} -> {next:?}");
        Ok(())
    }
}

pub(crate) fn show(
    app: &AppHandle,
    transition: &mut MainWindowTransition<'_>,
) -> Result<(), String> {
    if transition.mode() == MainWindowMode::Background {
        return Err("background window must be restored to hidden before showing".to_owned());
    }
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        log::error!("show failed: window missing");
        transition.move_to(MainWindowMode::Background)?;
        return Err("main window is unavailable".to_owned());
    };

    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    transition.move_to(MainWindowMode::Visible)?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn hide(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<MainWindowState>();
    let mut transition = state.begin_transition()?;
    if transition.mode() == MainWindowMode::Background {
        return Ok(());
    }
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        transition.move_to(MainWindowMode::Background)?;
        return Ok(());
    };
    if transition.mode() == MainWindowMode::Hidden {
        return Ok(());
    }

    window.hide().map_err(|error| error.to_string())?;
    transition.move_to(MainWindowMode::Hidden)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_window_modes() {
        assert_eq!(MainWindowMode::from_u8(0), MainWindowMode::Visible);
        assert_eq!(MainWindowMode::from_u8(1), MainWindowMode::Hidden);
        assert_eq!(MainWindowMode::from_u8(2), MainWindowMode::Background);
        assert_eq!(MainWindowMode::from_u8(u8::MAX), MainWindowMode::Hidden);
    }

    #[test]
    fn rejects_direct_restore() {
        assert!(MainWindowMode::Background.allows(MainWindowMode::Hidden));
        assert!(MainWindowMode::Hidden.allows(MainWindowMode::Visible));
        assert!(!MainWindowMode::Background.allows(MainWindowMode::Visible));
    }
}

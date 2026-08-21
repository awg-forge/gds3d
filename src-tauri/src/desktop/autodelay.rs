use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub(crate) struct AutoDelay {
    generation: Arc<AtomicU64>,
}

impl AutoDelay {
    pub(crate) fn new() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) fn cancel(&self) {
        self.advance();
    }

    pub(crate) fn schedule(
        &self,
        delay: Duration,
        action: impl FnOnce(DelayTicket) + Send + 'static,
    ) {
        let ticket = DelayTicket {
            generation: self.advance(),
            current: Arc::clone(&self.generation),
        };
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(delay).await;
            if ticket.is_current() {
                action(ticket);
            }
        });
    }

    fn advance(&self) -> u64 {
        self.generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
    }
}

pub(crate) struct DelayTicket {
    generation: u64,
    current: Arc<AtomicU64>,
}

impl DelayTicket {
    pub(crate) fn is_current(&self) -> bool {
        self.current.load(Ordering::Acquire) == self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidates_old_ticket() {
        let delay = AutoDelay::new();
        let ticket = DelayTicket {
            generation: delay.advance(),
            current: Arc::clone(&delay.generation),
        };

        assert!(ticket.is_current());
        delay.cancel();
        assert!(!ticket.is_current());
    }
}

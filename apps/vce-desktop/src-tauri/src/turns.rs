//! One active conversation, bounded waiting, and interruption shared by text/voice.
use std::{future::Future, time::Duration};
use tokio::sync::{watch, Mutex};

pub struct ConversationTurns {
    active: Mutex<()>,
    revision: watch::Sender<u64>,
}
impl Default for ConversationTurns {
    fn default() -> Self {
        let (revision, _) = watch::channel(0);
        Self {
            active: Mutex::new(()),
            revision,
        }
    }
}
impl ConversationTurns {
    pub fn cancel(&self) {
        self.revision
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }
    pub async fn cancel_and_wait(&self) {
        self.cancel();
        // Wait for abort cleanup before accepting the next voice transcript.
        let _idle = self.active.lock().await;
    }
    #[cfg(test)]
    pub async fn run<T>(
        &self,
        timeout: Duration,
        work: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        self.run_with_cleanup(timeout, work, || {}).await
    }
    pub async fn run_with_cleanup<T>(
        &self,
        timeout: Duration,
        work: impl Future<Output = Result<T, String>>,
        abort: impl FnOnce(),
    ) -> Result<T, String> {
        let _active = self.active.try_lock().map_err(|_| {
            "BestBuddy is finishing a conversation. Interrupt it before starting another."
                .to_string()
        })?;
        let mut revision = self.revision.subscribe();
        let result = tokio::select! {
            biased;
            _ = revision.changed() => { abort(); return Err("Conversation interrupted.".into()); },
            result = tokio::time::timeout(timeout,work) => result,
        };
        match result {
            Ok(reply) => reply,
            Err(_) => {
                abort();
                Err("Conversation timed out; please try a shorter request.".into())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn interruption_drops_work_and_releases_slot() {
        let turns = ConversationTurns::default();
        let work = turns.run(Duration::from_secs(1), async {
            std::future::pending::<()>().await;
            Ok::<_, String>(())
        });
        let cancel = async {
            tokio::task::yield_now().await;
            turns.cancel();
        };
        let (result, ()) = tokio::join!(work, cancel);
        assert_eq!(result.unwrap_err(), "Conversation interrupted.");
        assert_eq!(
            turns
                .run(Duration::from_secs(1), async { Ok(7) })
                .await
                .unwrap(),
            7
        );
    }
    #[tokio::test]
    async fn timeout_and_concurrent_requests_are_bounded() {
        let turns = ConversationTurns::default();
        let slow = turns.run(Duration::from_millis(10), async {
            std::future::pending::<()>().await;
            Ok::<_, String>(())
        });
        let other = async {
            tokio::task::yield_now().await;
            turns.run(Duration::from_secs(1), async { Ok(1) }).await
        };
        let (slow, other) = tokio::join!(slow, other);
        assert!(slow.unwrap_err().contains("timed out"));
        assert!(other.unwrap_err().contains("finishing"));
    }
}

#[cfg(test)]
mod abort_order_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[tokio::test]
    async fn cancellation_waits_for_cleanup_and_idle_cancel_keeps_warm_resources() {
        let turns = ConversationTurns::default();
        let aborted = AtomicBool::new(false);
        turns.cancel_and_wait().await;
        assert!(!aborted.load(Ordering::SeqCst));
        let work = turns.run_with_cleanup(
            Duration::from_secs(1),
            async {
                std::future::pending::<()>().await;
                Ok::<_, String>(())
            },
            || {
                aborted.store(true, Ordering::SeqCst);
            },
        );
        let cancel = async {
            tokio::task::yield_now().await;
            turns.cancel_and_wait().await;
            assert!(aborted.load(Ordering::SeqCst));
        };
        let (result, ()) = tokio::join!(work, cancel);
        assert!(result.is_err());
    }
}

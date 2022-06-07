use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

#[derive(Clone)]
#[doc(hidden)]
pub struct ShutdownToken {
    token: CancellationToken,
    is_toplevel: bool,
}

pub fn create_shutdown_token() -> ShutdownToken {
    ShutdownToken {
        token: CancellationToken::new(),
        is_toplevel: true,
    }
}

impl ShutdownToken {
    pub fn shutdown(&self) {
        if !self.token.is_cancelled() {
            if self.is_toplevel {
                log::info!("Initiating shutdown ...");
            } else {
                log::debug!("Initiating partial shutdown ...");
            }
            self.token.cancel()
        }
    }

    pub fn wait_for_shutdown(&self) -> WaitForCancellationFuture<'_> {
        self.token.cancelled()
    }

    pub fn is_shutting_down(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn child_token(&self) -> Self {
        Self {
            token: self.token.child_token(),
            is_toplevel: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn triggers_correctly() {
        let finished = AtomicBool::new(false);

        let token1 = create_shutdown_token();
        let token2 = token1.clone();

        let stoppee = async {
            token2.wait_for_shutdown().await;
            finished.store(true, Ordering::SeqCst);
        };

        let stopper = async {
            sleep(Duration::from_millis(100)).await;
            assert!(!finished.load(Ordering::SeqCst));
            assert!(!token1.is_shutting_down());
            assert!(!token2.is_shutting_down());

            token1.shutdown();
            sleep(Duration::from_millis(100)).await;

            assert!(finished.load(Ordering::SeqCst));
            assert!(token1.is_shutting_down());
            assert!(token2.is_shutting_down());
        };

        tokio::join!(stopper, stoppee);
    }

    #[tokio::test]
    async fn triggers_correctly_on_partial() {
        let finished = AtomicBool::new(false);

        let token1 = create_shutdown_token().child_token();
        let token2 = token1.clone();

        let stoppee = async {
            token2.wait_for_shutdown().await;
            finished.store(true, Ordering::SeqCst);
        };

        let stopper = async {
            sleep(Duration::from_millis(100)).await;
            assert!(!finished.load(Ordering::SeqCst));
            assert!(!token1.is_shutting_down());
            assert!(!token2.is_shutting_down());

            token1.shutdown();
            sleep(Duration::from_millis(100)).await;

            assert!(finished.load(Ordering::SeqCst));
            assert!(token1.is_shutting_down());
            assert!(token2.is_shutting_down());
        };

        tokio::join!(stopper, stoppee);
    }

    #[tokio::test]
    async fn double_shutdown_causes_no_error() {
        let token1 = create_shutdown_token();
        let token2 = create_shutdown_token().child_token();

        token1.shutdown();
        token1.shutdown();
        token2.shutdown();
        token2.shutdown();

        assert!(token1.is_shutting_down());
        assert!(token2.is_shutting_down());
    }
}

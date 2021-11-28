use tokio_util::sync::CancellationToken;

#[derive(Clone)]
#[doc(hidden)]
pub struct ShutdownToken {
    token: CancellationToken,
}

pub fn create_shutdown_token() -> ShutdownToken {
    ShutdownToken {
        token: CancellationToken::new(),
    }
}

impl ShutdownToken {
    pub fn shutdown(&self) {
        if !self.token.is_cancelled() {
            log::info!("Initiating shutdown ...");
            self.token.cancel()
        }
    }

    pub fn partial_shutdown(&self) {
        if !self.token.is_cancelled() {
            log::debug!("Initiating partial shutdown ...");
            self.token.cancel()
        }
    }

    pub async fn wait_for_shutdown(&self) {
        self.token.cancelled().await
    }

    pub fn is_shutting_down(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn child_token(&self) -> Self {
        Self {
            token: self.token.child_token(),
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

            token1.partial_shutdown();
            sleep(Duration::from_millis(100)).await;

            assert!(finished.load(Ordering::SeqCst));
            assert!(token1.is_shutting_down());
            assert!(token2.is_shutting_down());
        };

        tokio::join!(stopper, stoppee);
    }
}

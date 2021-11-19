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

    pub async fn wait_for_shutdown(&self) {
        self.token.cancelled().await
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

        let token = create_shutdown_token();

        let stoppee = async {
            token.wait_for_shutdown().await;
            finished.store(true, Ordering::SeqCst);
        };

        let stopper = async {
            sleep(Duration::from_millis(100)).await;
            assert!(!finished.load(Ordering::SeqCst));
            token.shutdown();
            sleep(Duration::from_millis(100)).await;
            assert!(finished.load(Ordering::SeqCst));
        };

        tokio::join!(stopper, stoppee);
    }
}

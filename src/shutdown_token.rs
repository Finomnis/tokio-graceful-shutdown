use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ShutdownToken {
    token: CancellationToken,
}

impl ShutdownToken {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

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

use tokio_util::sync::CancellationToken;

#[derive(Clone)]
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

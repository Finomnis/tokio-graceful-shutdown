use lazy_static::lazy_static;
use tokio_util::sync::CancellationToken;

// Signals global shutdown
lazy_static! {
    static ref SHUTDOWN_TOKEN: CancellationToken = CancellationToken::new();
}

/// Waits asynchronously until a program shutdown was initiated
pub async fn wait_until_shutdown_started() {
    SHUTDOWN_TOKEN.cancelled().await;
}

/// Initiates a shutdown
pub fn initiate_shutdown() {
    log::info!("Initiating shutdown ...");
    SHUTDOWN_TOKEN.cancel();
}

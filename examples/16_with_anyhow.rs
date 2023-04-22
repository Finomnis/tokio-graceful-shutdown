//! This example shows how to use this library with anyhow instead of miette

use anyhow::{anyhow, Result};

use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

#[tracing::instrument(name = "Subsys1", skip_all)]
async fn subsys1(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(anyhow!("Subsystem1 threw an error."))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}

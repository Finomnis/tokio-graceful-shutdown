//! This subsystem demonstrates the shutdown timeout mechanism.
//!
//! The subsystem takes longer to shut down than the timeout allows,
//! so the subsystem gets cancelled and the program returns an appropriate
//! error code.

use miette::Result;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(2000)).await;
    tracing::info!("Subsystem1 stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("Subsys1", subsys1));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(500))
    .await
    .map_err(Into::into)
}

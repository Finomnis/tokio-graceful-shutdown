//! This example demonstrates that if one subsystem returns an error,
//! a graceful shutdown is performed and other subsystems get the chance
//! to clean up.

use miette::{miette, Result};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

#[tracing::instrument(name = "Subsys1", skip_all)]
async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    subsys.start("Subsys2", subsys2);
    tracing::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");
    Ok(())
}

#[tracing::instrument(name = "Subsys2", skip_all)]
async fn subsys2(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem2 started.");
    sleep(Duration::from_millis(500)).await;

    Err(miette!("Subsystem2 threw an error."))
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

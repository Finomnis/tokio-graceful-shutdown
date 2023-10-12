//! This example demonstrates that like errors, panics also get dealt with
//! gracefully.
//!
//! A normal program shutdown is performed, and other subsystems get the
//! chance to clean up their work.

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    subsys.start(SubsystemBuilder::new("Subsys2", subsys2));
    tracing::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");
    Ok(())
}

async fn subsys2(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem2 started.");
    sleep(Duration::from_millis(500)).await;

    panic!("Subsystem2 panicked!")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("Subsys1", subsys1));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}

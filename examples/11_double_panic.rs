//! This example demonstrates if a subsystem panics during a shutdown caused
//! by another panic, the shutdown is still performed normally and the third
//! subsystem gets cleaned up without a problem.
//!
//! Note that this even works when running in tokio's single-threaded mode.
//!
//! There is no real programming knowledge to be gained here, this example is just
//! to demonstrate the robustness of the system.

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    subsys.start(SubsystemBuilder::new("Subsys2", subsys2));
    subsys.start(SubsystemBuilder::new("Subsys3", subsys3));
    tracing::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(200)).await;
    panic!("Subsystem1 panicked!");
}

async fn subsys2(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem2 started.");
    sleep(Duration::from_millis(500)).await;

    panic!("Subsystem2 panicked!")
}

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem3 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem3 ...");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem3 shut down successfully.");
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
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

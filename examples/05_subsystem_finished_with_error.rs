//! This example shows how the library reacts to failing subsystems.
//!
//! If a subsystem returns an `Err(...)` value, it is assumed that the
//! subsystem failed and in response the program will be shut down.
//!
//! As expected, this is a graceful shutdown, giving other subsystems
//! the chance to also shut down gracefully.

use miette::{miette, Result};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

#[tracing::instrument(name = "Subsys1", skip_all)]
async fn subsys1(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(miette!("Subsystem1 threw an error."))
}

#[tracing::instrument(name = "Subsys2", skip_all)]
async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    subsys.on_shutdown_requested().await;
    Ok(())
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
        .start("Subsys2", subsys2)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}

//! This example demonstrates how to use the tokio-console application for tracing tokio tasks's
//! runtime behaviour. Subsystems will appear under their registration names.
//!
//! To make this work,
//!
//! * Compile `tokio-graceful-shutdown` with the `tokio-unstable` feature to register subsystem
//!   task names.
//!
//! * Run this example with the environment variable:
//!
//!   ```
//!   RUSTFLAGS=="--cfg tokio_unstable"
//!   ```
//!
//! * Run the `tokio-console` CLI application and watch your snappy low-latency tasks

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing_subscriber::prelude::*;

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    subsys.start(SubsystemBuilder::new("Subsys2", subsys2));
    tracing::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");
    Ok(())
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem2 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Shutting down Subsystem2 ...");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem2 stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let console_layer = console_subscriber::spawn();
    // Init logging
    tracing_subscriber::registry()
        .with(console_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("Subsys1", subsys1));
        s.start(SubsystemBuilder::new("Subsys2", subsys2));
        s.start(SubsystemBuilder::new("Subsys3", subsys1));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}

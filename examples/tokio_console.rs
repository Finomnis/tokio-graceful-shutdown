//! This example demonstrates how to use the tokio-console application for tracing tokio tasks's
//! runtime behaviour. Subsystems will appear under their registration names.
//!
//! Run this example with:
//!
//! ```
//! RUSTFLAGS="--cfg tokio_unstable" cargo run --features "tracing" --example tokio_console
//! ```
//!
//! Then, open the `tokio-console` application (see https://crates.io/crates/tokio-console) to
//! follow the subsystem tasks live.

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing::Level;
use tracing_subscriber::{fmt::writer::MakeWriterExt, prelude::*};

async fn child(subsys: SubsystemHandle) -> Result<()> {
    sleep(Duration::from_millis(3000))
        .cancel_on_shutdown(&subsys)
        .await
        .ok();
    Ok(())
}

async fn parent(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Parent started.");

    let mut iteration = 0;
    while !subsys.is_shutdown_requested() {
        subsys.start(SubsystemBuilder::new(format!("child{iteration}"), child));
        iteration += 1;

        sleep(Duration::from_millis(1000))
            .cancel_on_shutdown(&subsys)
            .await
            .ok();
    }

    tracing::info!("Parent stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init tokio-console server and tracing
    let console_layer = console_subscriber::spawn();
    tracing_subscriber::registry()
        .with(console_layer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout.with_max_level(Level::DEBUG))
                .compact(),
        )
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("parent", parent));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}

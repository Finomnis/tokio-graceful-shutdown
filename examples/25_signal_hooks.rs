//! This example demonstrates how to use `catch_signals_with_hooks` to react differently to various
//! OS signals that can trigger a shutdown.
//!
//! Run the example and then try sending it different signals:
//! - Press `Ctrl+C` in your terminal to send a `SIGINT`.
//! - From another terminal, run `kill <PID>` to send a `SIGTERM`.
//!
//! The custom `MySignalHooks` implementation will log a specific, verbose message for each signal,
//! showing how you can customize the behavior.

use anyhow::Result;
use async_trait::async_trait;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{SignalHooks, SubsystemBuilder, SubsystemHandle, Toplevel};

struct MySignalHooks;

#[async_trait]
impl SignalHooks for MySignalHooks {
    #[cfg(unix)]
    async fn on_sigterm(&mut self) {
        tracing::info!("Received SIGTERM. This might be from a service manager like systemd.");
        tracing::info!("Initiating a graceful shutdown...");
    }

    #[cfg(unix)]
    async fn on_sigint(&mut self) {
        tracing::info!("Received SIGINT, likely from Ctrl+C.");
        tracing::info!("Starting shutdown immediately!");
    }

    #[cfg(windows)]
    async fn on_ctrl_c(&mut self) {
        tracing::warn!("Received CTRL_C, likely from Ctrl+C.");
        tracing::warn!("Starting shutdown immediately!");
    }
}

async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("My subsystem started.");
    let pid = std::process::id();
    tracing::info!("My PID is: {}. Send me signals to test the hooks.", pid);
    tracing::info!("Waiting for shutdown signal...");
    subsys.on_shutdown_requested().await;
    tracing::info!("My subsystem is shutting down.");
    sleep(Duration::from_millis(500)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let result = Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("MySubsystem", my_subsystem));
    })
    .catch_signals_with_hooks(MySignalHooks)
    .handle_shutdown_requests(Duration::from_secs(1))
    .await;

    if let Err(e) = &result {
        tracing::error!("Application finished with an error: {}", e);
    } else {
        tracing::info!("Application finished successfully.");
    }

    result.map_err(Into::into)
}

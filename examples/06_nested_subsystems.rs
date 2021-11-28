//! This example demonstrates how one subsystem can launch another
//! nested subsystem.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(mut subsys: SubsystemHandle) -> Result<()> {
    subsys.start("Subsys2", subsys2);
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem2 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem2 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem2 stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}

//! This subsystem demonstrates the shutdown timeout mechanism.
//!
//! The subsystem takes longer to shut down than the timeout allows,
//! so the subsystem gets cancelled and the program returns an appropriate
//! error code.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(1000)).await;
    log::info!("Subsystem1 stopped.");
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
        .wait_for_shutdown(Duration::from_millis(500))
        .await
}

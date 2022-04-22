//! This example shows how to use this library with miette instead of anyhow

use env_logger::{Builder, Env};
use miette::{miette, Result};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{GracefulShutdownError, SubsystemHandle, Toplevel};

async fn subsys1(_subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(miette!("Subsystem1 threw an error."))
}

#[tokio::main]
async fn main() -> Result<(), GracefulShutdownError> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}

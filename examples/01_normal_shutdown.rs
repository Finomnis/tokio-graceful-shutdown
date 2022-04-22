//! This example demonstrates the basic usage pattern of this crate.
//!
//! It shows that subsystems get started, and when the program
//! gets shut down (by pressing Ctrl-C), the subsystems get shut down
//! gracefully.
//!
//! If custom arguments for the subsystem coroutines are required,
//! a struct has to be used instead, as seen in other examples.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{GracefulShutdownError, SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(400)).await;
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
async fn main() -> Result<(), GracefulShutdownError> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .start("Subsys2", subsys2)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}

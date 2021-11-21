//! This example demonstrates if a subsystem panics during a shutdown caused
//! by another panic, the shutdown is still performed normally and the third
//! subsystem gets cleaned up without a problem.
//!
//! Note that this even works when running in tokio's single-threaded mode.
//!
//! There is no real programming knowledge to be gained here, this example is just
//! to demonstrate the robustness of the system.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(mut subsys: SubsystemHandle) -> Result<()> {
    subsys.start("Subsys2", subsys2);
    subsys.start("Subsys3", subsys3);
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(200)).await;
    panic!("Subsystem1 panicked!");
}

async fn subsys2(_subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem2 started.");
    sleep(Duration::from_millis(500)).await;

    panic!("Subsystem2 panicked!")
}

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem3 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem3 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem3 shut down successfully.");
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}

//! This example demonstrates that like errors, panics also get dealt with
//! gracefully.
//!
//! A normal program shutdown is performed, and other subsystems get the
//! chance to clean up their work.

use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{Error, SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<(), Error> {
    subsys.start("Subsys2", subsys2);
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

async fn subsys2(_subsys: SubsystemHandle) -> Result<(), Error> {
    log::info!("Subsystem2 started.");
    sleep(Duration::from_millis(500)).await;

    panic!("Subsystem2 panicked!")
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
}

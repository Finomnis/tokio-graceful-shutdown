//! This example shows how the library reacts to failing subsystems.
//!
//! If a subsystem returns an `Err(...)` value, it is assumed that the
//! subsystem failed and in response the program will be shut down.
//!
//! As expected, this is a graceful shutdown, giving other subsystems
//! the chance to also shut down gracefully.

use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{Error, SubsystemHandle, Toplevel};

async fn subsys1(_subsys: SubsystemHandle) -> Result<(), Error> {
    log::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");

    // Task ends with an error. This should cause the main program to shutdown.
    Err(anyhow::anyhow!("Subsystem1 threw an error.").into())
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

//! This subsystem demonstrates that subsystems can also stop
//! prematurely.
//!
//! Returning Ok(()) from a subsystem indicates that the subsystem
//! stopped intentionally, and no further measures by the runtime are performed.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{GracefulShutdownError, SubsystemHandle, Toplevel};

async fn subsys1(_subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");

    // Task ends without an error. This should not cause the main program to shutdown.
    Ok(())
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

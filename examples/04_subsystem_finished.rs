//! This subsystem demonstrates that subsystems can also stop
//! prematurely.
//!
//! Returning Ok(()) from a subsystem indicates that the subsystem
//! stopped intentionally, and no further measures by the runtime are performed.
//! (unless there are no more subsystems left, in that case TopLevel would shut down anyway)

use env_logger::{Builder, Env};
use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(_subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    log::info!("Subsystem1 stopped.");

    // Task ends without an error. This should not cause the main program to shutdown,
    // because Subsys2 is still running.
    Ok(())
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    subsys.on_shutdown_requested().await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
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

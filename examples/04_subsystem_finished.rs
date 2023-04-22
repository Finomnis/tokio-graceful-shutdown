//! This subsystem demonstrates that subsystems can also stop
//! prematurely.
//!
//! Returning Ok(()) from a subsystem indicates that the subsystem
//! stopped intentionally, and no further measures by the runtime are performed.
//! (unless there are no more subsystems left, in that case TopLevel would shut down anyway)

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

#[tracing::instrument(name = "Subsys1", skip_all)]
async fn subsys1(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem1 started.");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem1 stopped.");

    // Task ends without an error. This should not cause the main program to shutdown,
    // because Subsys2 is still running.
    Ok(())
}

#[tracing::instrument(name = "Subsys2", skip_all)]
async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    subsys.on_shutdown_requested().await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Create toplevel
    Toplevel::new()
        .start("Subsys1", subsys1)
        .start("Subsys2", subsys2)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}

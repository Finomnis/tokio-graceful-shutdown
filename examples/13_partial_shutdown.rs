//! This example demonstrates how to perform a partial shutdown of the system.
//!
//! Subsys1 will perform a partial shutdown after 5 seconds, which will in turn
//! shut down Subsys2 and Subsys3, leaving Subsys1 running.

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{ErrorAction, SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsys3 started.");
    subsys.on_shutdown_requested().await;
    tracing::info!("Subsys3 stopped.");
    Ok(())
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsys2 started.");
    subsys.start(SubsystemBuilder::new("Subsys3", subsys3));
    subsys.on_shutdown_requested().await;
    tracing::info!("Subsys2 stopped.");
    Ok(())
}

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    // This subsystem shuts down the nested subsystem after 5 seconds.
    tracing::info!("Subsys1 started.");

    tracing::info!("Starting nested subsystem ...");
    let nested_subsys = subsys.start(SubsystemBuilder::new("Subsys2", subsys2));
    tracing::info!("Nested subsystem started.");

    tokio::select! {
        _ = subsys.on_shutdown_requested() => (),
        _ = sleep(Duration::from_secs(1)) => {
            tracing::info!("Shutting down nested subsystem ...");
            // Redirect errors during shutdown to the local `.join()` call
            nested_subsys.change_failure_action(ErrorAction::CatchAndLocalShutdown);
            nested_subsys.change_panic_action(ErrorAction::CatchAndLocalShutdown);
            // Perform shutdown
            nested_subsys.initiate_shutdown();
            nested_subsys.join().await?;
            tracing::info!("Nested subsystem shut down.");
            subsys.on_shutdown_requested().await;
        }
    };

    tracing::info!("Subsys1 stopped.");

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("Subsys1", subsys1));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}

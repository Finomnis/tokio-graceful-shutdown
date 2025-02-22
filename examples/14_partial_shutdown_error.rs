//! This example demonstrates how an error during partial shutdown behaves.
//!
//! If an error during partial a shutdown happens, it will not cause a global
//! shutdown, but instead it will be delivered to the task that initiated
//! the partial shutdown.

use miette::Result;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{ErrorAction, SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsys3 started.");
    subsys.on_shutdown_requested().await;
    panic!("Subsystem3 threw an error!")
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
            if let Err(err) = nested_subsys.join().await {
                tracing::warn!("Error during nested subsystem shutdown: {:?}", miette::Report::from(err));
            };
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
        .with_max_level(tracing::Level::DEBUG)
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

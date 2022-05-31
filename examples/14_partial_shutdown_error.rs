//! This example demonstrates how an error during partial shutdown behaves.
//!
//! If an error during partial a shutdown happens, it will not cause a global
//! shutdown, but instead it will be delivered to the task that initiated
//! the partial shutdown.

use env_logger::{Builder, Env};
use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsys3 started.");
    subsys.on_shutdown_requested().await;
    panic!("Subsystem3 threw an error!")
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsys2 started.");
    subsys.start("Subsys3", subsys3);
    subsys.on_shutdown_requested().await;
    log::info!("Subsys2 stopped.");
    Ok(())
}

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    // This subsystem shuts down the nested subsystem after 5 seconds.
    log::info!("Subsys1 started.");

    log::info!("Starting nested subsystem ...");
    let nested_subsys = subsys.start("Subsys2", subsys2);
    log::info!("Nested subsystem started.");

    tokio::select! {
        _ = subsys.on_shutdown_requested() => (),
        _ = sleep(Duration::from_secs(1)) => {
            log::info!("Shutting down nested subsystem ...");
            if let Err(err) = subsys.perform_partial_shutdown(nested_subsys).await{
                log::warn!("Partial shutdown failed: {}", err);
            };
            log::info!("Nested subsystem shut down.");
            subsys.on_shutdown_requested().await;
        }
    };

    log::info!("Subsys1 stopped.");

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
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
}

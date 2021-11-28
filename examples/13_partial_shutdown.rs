//! This example demonstrates how to perform a partial shutdown of the system.
//!
//! Subsys1 will perform a partial shutdown after 5 seconds, which will in turn
//! shut down Subsys2 and Subsys3, leaving Subsys1 running.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsys3 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Subsys3 stopped.");
    Ok(())
}

async fn subsys2(mut subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsys2 started.");
    subsys.start("Subsys3", subsys3);
    subsys.on_shutdown_requested().await;
    log::info!("Subsys2 stopped.");
    Ok(())
}

async fn subsys1(mut subsys: SubsystemHandle) -> Result<()> {
    // This subsystem shuts down the nested subsystem after 5 seconds.
    log::info!("Subsys1 started.");

    log::info!("Starting nested subsystem ...");
    let nested_subsys = subsys.start("Subsys2", subsys2);
    log::info!("Nested subsystem started.");

    tokio::select! {
        _ = subsys.on_shutdown_requested() => (),
        _ = sleep(Duration::from_secs(5)) => {
            log::info!("Shutting down nested subsystem ...");
            subsys.perform_partial_shutdown(nested_subsys).await?;
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
}

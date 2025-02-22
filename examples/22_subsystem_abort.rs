//! This example demonstrates how a subsystem that is stuck (in an await) can get aborted.

use miette::Result;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem1 started.");
    let nested = subsys.start(SubsystemBuilder::new("Subsys2", subsys2));
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Aborting nested subsystem ...");
    nested.abort();
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Nested subsystem is finished: {:?}", nested.is_finished());
    Ok(())
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem2 started.");
    subsys.start(SubsystemBuilder::new("Subsys3", subsys3));
    loop {
        tracing::info!("Subsystem2 stuck ...");
        sleep(Duration::from_millis(100)).await;
    }
}

async fn subsys3(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem3 started.");
    loop {
        tracing::info!("Subsystem3 stuck ...");
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::main]
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

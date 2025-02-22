//! This example demonstrates how a parent subsystem could orchestrate
//! the shutdown order of its children manually.
//!
//! This is done by spawning the children in 'detached' mode to prevent
//! that the shutdown signal gets passed to the children.
//! Then, the parent calls `initialize_shutdown` on each child manually.

use miette::Result;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel};

async fn counter(id: &str) {
    let mut i = 0;
    loop {
        tracing::info!("{id}: {i}");
        i += 1;
        sleep(Duration::from_millis(50)).await;
    }
}

async fn child(name: &str, subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("{name} started.");
    if counter(name).cancel_on_shutdown(&subsys).await.is_ok() {
        tracing::info!("{name} counter finished.");
    } else {
        tracing::info!("{name} shutting down ...");
        sleep(Duration::from_millis(200)).await;
    }
    subsys.on_shutdown_requested().await;
    tracing::info!("{name} stopped.");
    Ok(())
}

async fn parent(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Parent started.");

    tracing::info!("Starting detached nested subsystems ...");
    let nested1 =
        subsys.start(SubsystemBuilder::new("Nested1", |s| child("Nested1", s)).detached());
    let nested2 =
        subsys.start(SubsystemBuilder::new("Nested2", |s| child("Nested2", s)).detached());
    let nested3 =
        subsys.start(SubsystemBuilder::new("Nested3", |s| child("Nested3", s)).detached());
    tracing::info!("Nested subsystems started.");

    // Wait for the shutdown to happen
    subsys.on_shutdown_requested().await;

    // Shut down children sequentially. As they are detached, they will not shutdown on their own,
    // but need to be shut down manually via `initiate_shutdown`.
    tracing::info!("Initiating Nested1 shutdown ...");
    nested1.initiate_shutdown();
    nested1.join().await?;
    tracing::info!("Initiating Nested2 shutdown ...");
    nested2.initiate_shutdown();
    nested2.join().await?;
    tracing::info!("Initiating Nested3 shutdown ...");
    nested3.initiate_shutdown();
    nested3.join().await?;

    tracing::info!("All children finished, stopping Root ...");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Root stopped.");

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("parent", parent));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}

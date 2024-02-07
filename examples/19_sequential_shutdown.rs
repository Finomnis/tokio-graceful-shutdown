//! This example demonstrates how multiple subsystems could be shut down sequentially.
//!
//! When a shutdown gets triggered (via Ctrl+C), Nested1 will shutdown first,
//! followed by Nested2 and Nested3. Only once the previous subsystem is finished shutting down,
//! the next subsystem will follow.

use miette::Result;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    FutureExt, SubsystemBuilder, SubsystemFinishedFuture, SubsystemHandle, Toplevel,
};

async fn counter(id: &str) {
    let mut i = 0;
    loop {
        tracing::info!("{id}: {i}");
        i += 1;
        sleep(Duration::from_millis(50)).await;
    }
}

async fn nested1(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Nested1 started.");
    if counter("Nested1").cancel_on_shutdown(&subsys).await.is_ok() {
        tracing::info!("Nested1 counter finished.");
    } else {
        tracing::info!("Nested1 shutting down ...");
        sleep(Duration::from_millis(200)).await;
    }
    subsys.on_shutdown_requested().await;
    tracing::info!("Nested1 stopped.");
    Ok(())
}

async fn nested2(subsys: SubsystemHandle, nested1_finished: SubsystemFinishedFuture) -> Result<()> {
    // Create a future that triggers once nested1 is finished **and** a shutdown is requested
    let shutdown = {
        let shutdown_requested = subsys.on_shutdown_requested();
        async move {
            tokio::join!(shutdown_requested, nested1_finished);
        }
    };

    tracing::info!("Nested2 started.");
    tokio::select! {
        _ = shutdown => {
            tracing::info!("Nested2 shutting down ...");
            sleep(Duration::from_millis(200)).await;
        }
        _ = counter("Nested2") => {
            tracing::info!("Nested2 counter finished.");
        }
    }

    tracing::info!("Nested2 stopped.");
    Ok(())
}

async fn nested3(subsys: SubsystemHandle, nested2_finished: SubsystemFinishedFuture) -> Result<()> {
    // Create a future that triggers once nested2 is finished **and** a shutdown is requested
    let shutdown = {
        // This is an alternative to `on_shutdown_requested()` (as shown in nested2).
        // Use this if `on_shutdown_requested()` gives you lifetime issues.
        let cancellation_token = subsys.create_cancellation_token();
        async move {
            tokio::join!(cancellation_token.cancelled(), nested2_finished);
        }
    };

    tracing::info!("Nested3 started.");
    tokio::select! {
        _ = shutdown => {
            tracing::info!("Nested3 shutting down ...");
            sleep(Duration::from_millis(200)).await;
        }
        _ = counter("Nested3") => {
            tracing::info!("Nested3 counter finished.");
        }
    }

    tracing::info!("Nested3 stopped.");
    Ok(())
}

async fn root(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Root started.");

    tracing::info!("Starting nested subsystems ...");
    let nested1 = subsys.start(SubsystemBuilder::new("Nested1", nested1));
    let nested1_finished = nested1.finished();
    let nested2 = subsys.start(SubsystemBuilder::new("Nested2", |s| {
        nested2(s, nested1_finished)
    }));
    let nested2_finished = nested2.finished();
    subsys.start(SubsystemBuilder::new("Nested3", |s| {
        nested3(s, nested2_finished)
    }));
    tracing::info!("Nested subsystems started.");

    // Wait for all children to finish shutting down.
    subsys.wait_for_children().await;

    tracing::info!("All children finished, stopping Root ...");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Root stopped.");

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
        s.start(SubsystemBuilder::new("Root", root));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}

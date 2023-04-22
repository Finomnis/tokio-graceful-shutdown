//! This example demonstrates how to gracefully shutdown a warp
//! server using this crate.
//!
//! Note that we have to wait for a long time in `handle_shutdown_requests` because
//! warp's graceful shutdown waits for all connections to be closed naturally
//! instead of terminating them.

use miette::Result;
use tokio::time::Duration;
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

use warp::Filter;

#[tracing::instrument(name = "Warp Subsys", skip_all)]
async fn warp_subsystem(subsys: SubsystemHandle) -> Result<()> {
    // Match any request and return hello world!
    let routes = warp::any().map(|| "Hello, World!");

    let (addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 12345), async move {
            subsys.on_shutdown_requested().await;
            tracing::info!("Starting server shutdown ...");
        });

    tracing::info!("Listening on http://{}", addr);

    server.await;

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
        .start("Warp", warp_subsystem)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_secs(60))
        .await
        .map_err(Into::into)
}

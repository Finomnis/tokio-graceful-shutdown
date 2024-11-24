//! This example demonstrates how to gracefully shutdown a hyper
//! server using this crate.
//!
//! This example closely follows hyper's "hello" example.
//!
//! Note that while we could spawn one subsystem per connection,
//! tokio-graceful-shutdown's subsystems are quite heavy.
//! So for a large amount of dynamic tasks like this, it is
//! recommended to use CancellationToken + TaskTracker instead.

use miette::{Context, IntoDiagnostic, Result};
use tokio::time::Duration;
use tokio_graceful_shutdown::errors::CancelledByShutdown;
use tokio_graceful_shutdown::{FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel};

use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::pin;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio_util::task::TaskTracker;

// An async function that consumes a request, does nothing with it and returns a
// response.
async fn hello(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello World!"))))
}

async fn connection_handler(
    subsys: SubsystemHandle,
    listener: TcpListener,
    connection_tracker: TaskTracker,
) -> Result<()> {
    loop {
        let connection = match listener.accept().cancel_on_shutdown(&subsys).await {
            Ok(connection) => connection,
            Err(CancelledByShutdown) => break,
        };
        let (tcp, addr) = connection
            .into_diagnostic()
            .context("Error while waiting for connection")?;
        let io = TokioIo::new(tcp);

        // Spawn handler on connection tracker to give the parent subsystem
        // the chance to wait for the shutdown to finish
        connection_tracker.spawn({
            let cancellation_token = subsys.create_cancellation_token();
            async move {
                tracing::info!("Connected to {} ...", addr);

                let mut connection =
                    pin!(http1::Builder::new().serve_connection(io, service_fn(hello)));

                let result = tokio::select! {
                    e = connection.as_mut() => e,
                    _ = cancellation_token.cancelled() => {
                        // If the system shuts down, shut down the connection
                        // and continue serving, as specified in the hyper docs.
                        tracing::info!("Shutting down connection to {} ...", addr);
                        connection.as_mut().graceful_shutdown();
                        connection.await
                    },
                };

                if let Err(err) = result {
                    tracing::warn!("Error serving connection: {:?}", err);
                } else {
                    tracing::info!("Connection to {} closed.", addr);
                }
            }
        });
    }

    Ok(())
}

async fn hyper_subsystem(subsys: SubsystemHandle) -> Result<()> {
    let addr: SocketAddr = ([127, 0, 0, 1], 12345).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr)
        .await
        .into_diagnostic()
        .context("Unable to start tcp server")?;
    tracing::info!("Listening on http://{}", addr);

    // Use a tasktracker instead of spawning a subsystem for every connection,
    // as this would result in a lot of overhead.
    let connection_tracker = TaskTracker::new();

    let listener = subsys.start(SubsystemBuilder::new("Hyper Listener", {
        let connection_tracker = connection_tracker.clone();
        move |subsys| connection_handler(subsys, listener, connection_tracker)
    }));

    // Make sure no more tasks can be spawned before we close the tracker
    listener.join().await?;

    // Wait for connections to close
    connection_tracker.close();
    connection_tracker.wait().await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("Hyper", hyper_subsystem));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_secs(5))
    .await
    .map_err(Into::into)
}

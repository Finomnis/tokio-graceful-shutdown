//! This example demonstrates how to gracefully shutdown a server
//! that spawns an indefinite number of connection tasks.
//!
//! The server is a simple TCP echo server, capitalizing the data
//! it echos (to demonstrate that it computes things).
//! On shutdown, it transmits a goodbye message, to demonstrate
//! that during shutdown we can still perform cleanup steps.
//!
//! This example is similar to the hyper example; for a more complex
//! version of this same example, look there.

use miette::{Context, IntoDiagnostic, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Duration;
use tokio_graceful_shutdown::errors::CancelledByShutdown;
use tokio_graceful_shutdown::{FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel};

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tokio_util::task::TaskTracker;

async fn echo_connection(tcp: &mut TcpStream) -> Result<()> {
    tcp.write_all(b"Hello!\r\n").await.into_diagnostic()?;

    let mut buffer = [0u8; 256];
    loop {
        match tcp.read(&mut buffer).await {
            Ok(0) => return Ok(()),
            Err(e) => return Err(e).into_diagnostic(),
            Ok(len) => {
                let bytes = &mut buffer[..len];
                for byte in bytes.iter_mut() {
                    *byte = byte.to_ascii_uppercase();
                }
                tcp.write_all(bytes).await.into_diagnostic()?;
            }
        }
    }
}

async fn echo_connection_shutdown(tcp: &mut TcpStream) -> Result<()> {
    tcp.write_all(b"Goodbye.\r\n").await.into_diagnostic()?;
    tcp.shutdown().await.into_diagnostic()?;

    Ok(())
}

async fn connection_handler(
    subsys: &mut SubsystemHandle,
    listener: TcpListener,
    connection_tracker: TaskTracker,
) -> Result<()> {
    loop {
        let connection = match listener.accept().cancel_on_shutdown(subsys).await {
            Ok(connection) => connection,
            Err(CancelledByShutdown) => break,
        };
        let (mut tcp, addr) = connection
            .into_diagnostic()
            .context("Error while waiting for connection")?;

        // Spawn handler on connection tracker to give the parent subsystem
        // the chance to wait for the shutdown to finish
        connection_tracker.spawn({
            let cancellation_token = subsys.create_cancellation_token();
            async move {
                tracing::info!("Connected to {} ...", addr);

                let result = tokio::select! {
                    e = echo_connection(&mut tcp) => e,
                    _ = cancellation_token.cancelled() => {
                        tracing::info!("Shutting down {} ...", addr);
                        echo_connection_shutdown(&mut tcp).await
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

async fn echo_subsystem(subsys: &mut SubsystemHandle) -> Result<()> {
    let addr: SocketAddr = ([127, 0, 0, 1], 12345).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr)
        .await
        .into_diagnostic()
        .context("Unable to start tcp server")?;
    tracing::info!("Listening on {}", addr);

    // Use a tasktracker instead of spawning a subsystem for every connection,
    // as this would result in a lot of overhead.
    let connection_tracker = TaskTracker::new();

    let listener = subsys.start(SubsystemBuilder::new("Echo Listener", {
        let connection_tracker = connection_tracker.clone();
        async move |subsys: &mut SubsystemHandle| {
            connection_handler(subsys, listener, connection_tracker).await
        }
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
    Toplevel::new(async |s: &mut SubsystemHandle| {
        s.start(SubsystemBuilder::new("EchoServer", echo_subsystem));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_secs(5))
    .await
    .map_err(Into::into)
}

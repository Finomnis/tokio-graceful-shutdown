//! This example demonstrates how the entire tokio runtime can be run
//! in its own thread and how the subsystem tree can be then shut down
//! from another thread thread.

use miette::{Result, miette};
use tokio::{
    runtime::Runtime,
    time::{Duration, sleep},
};
use tokio_graceful_shutdown::{FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel};
use tokio_util::sync::CancellationToken;

async fn counter(subsys: SubsystemHandle) -> Result<()> {
    let mut i = 1;
    while !subsys.is_shutdown_requested() {
        tracing::info!("Counter: {}", i);
        sleep(Duration::from_millis(1000))
            .cancel_on_shutdown(&subsys)
            .await
            .ok();

        i += 1;
    }

    tracing::info!("Counter stopped.");

    Ok(())
}

fn tokio_thread(shutdown_token: CancellationToken) -> Result<()> {
    Runtime::new().unwrap().block_on(async {
        // Setup and execute subsystem tree
        Toplevel::new_with_shutdown_token(
            async |s| {
                s.start(SubsystemBuilder::new("Counter", counter));
            },
            shutdown_token,
        )
        .handle_shutdown_requests(Duration::from_millis(1000))
        .await
        .map_err(Into::into)
    })
}

fn main() -> Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let shutdown_token = CancellationToken::new();

    let tokio_thread_handle = std::thread::spawn({
        let shutdown_token = shutdown_token.clone();
        move || tokio_thread(shutdown_token)
    });

    std::thread::sleep(Duration::from_millis(4500));

    tracing::info!("Initiating shutdown ...");
    shutdown_token.cancel();

    match tokio_thread_handle.join() {
        Ok(result) => {
            tracing::info!("Shutdown finished.");
            result
        }
        Err(_) => Err(miette!("Error while waiting for tokio thread!")),
    }
}

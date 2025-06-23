//! This example demonstrates how to use `Toplevel::new_with_hook` to run a custom callback
//! immediately when an uncaught error occurs.
//!
//! The `FailingSubsystem` will return an error after 500ms.
//! This triggers the `on_subsystem_error` hook instantly, which logs the error to a shared vector.
//!
//! Simultaneously, a global shutdown is initiated. The `LongRunningSubsystem` receives this
//! shutdown signal and performs a slow cleanup.

use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{
    SubsystemBuilder, SubsystemHandle, Toplevel, errors::SubsystemError,
};

async fn failing_subsystem(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("FailingSubsystem started. Will fail in 500ms.");
    sleep(Duration::from_millis(500)).await;
    Err(anyhow!("FailingSubsystem failed as planned."))
}

async fn long_running_subsystem(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("LongRunningSubsystem started, waiting for shutdown...");
    subsys.on_shutdown_requested().await;
    tracing::info!("LongRunningSubsystem shutting down (will take 1s)...");
    sleep(Duration::from_secs(1)).await;
    tracing::info!("LongRunningSubsystem finished cleanup.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let hook_events = Arc::new(Mutex::new(Vec::new()));
    let hook_events_clone = Arc::clone(&hook_events);

    let on_subsystem_error = move |error: &SubsystemError| {
        let msg = format!(
            "FATAL ERROR HOOK: Subsystem '{}' failed: {}",
            error.name(),
            error
        );

        eprintln!("{}", msg);
        hook_events_clone.lock().unwrap().push(msg);
    };

    let toplevel = Toplevel::new_with_hook(
        async move |s| {
            s.start(SubsystemBuilder::new("Failing", failing_subsystem));
            s.start(SubsystemBuilder::new("LongRunning", long_running_subsystem));
        },
        on_subsystem_error,
    );

    let result = toplevel
        .catch_signals()
        .handle_shutdown_requests(Duration::from_secs(5))
        .await;

    println!("\n-- Toplevel Error Hook Report --");
    for event in hook_events.lock().unwrap().iter() {
        println!("- {}", event);
    }
    println!("--------------------------------\n");

    if let Err(e) = &result {
        tracing::error!("Application finished with an error (as expected): {}", e);
    }

    Ok(())
}

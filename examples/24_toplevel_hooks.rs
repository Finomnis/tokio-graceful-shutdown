//! This example demonstrates how to use `Toplevel::new_with_hooks` to run custom callbacks for two
//! key top-level events:
//!
//! 1.  An uncaught error from a subsystem.
//! 2.  The cancellation of the root subsystem itself.
//!
//! Run with: `cargo run --example 24_toplevel_hooks [SCENARIO]`
//!
//! Where `[SCENARIO]` can be one of:
//!   - `error` (default): A subsystem fails, triggering the `on_subsystem_error` hook immediately.
//!   - `cancel`: The `Toplevel` object is dropped prematurely, cancelling the root
//!               subsystem and triggering the `on_subsystem_cancelled` hook.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    default_on_subsystem_cancelled, default_on_subsystem_error, errors::SubsystemError,
    SubsystemBuilder, SubsystemHandle, Toplevel,
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

async fn endless_subsystem(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("EndlessSubsystem started. It will run until cancelled.");
    sleep(Duration::from_secs(10)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let scenario = std::env::args().nth(1).unwrap_or("error".to_string());
    tracing::info!("Running scenario: '{}'", scenario);

    let hook_events = Arc::new(Mutex::new(Vec::new()));

    match scenario.as_str() {
        "error" => {
            let on_subsystem_error = {
                let hook_events = hook_events.clone();
                move |error: &SubsystemError| {
                    let msg = format!(
                        "FATAL ERROR HOOK: Subsystem '{}' failed: {}",
                        error.name(),
                        error
                    );
                    eprintln!("{}", msg);
                    hook_events.lock().unwrap().push(msg);
                }
            };

            let toplevel = Toplevel::new_with_hooks(
                async move |s| {
                    s.start(SubsystemBuilder::new("Failing", failing_subsystem));
                    s.start(SubsystemBuilder::new("LongRunning", long_running_subsystem));
                },
                on_subsystem_error,
                // We don't expect cancellation, so we use the default hook.
                default_on_subsystem_cancelled,
            );

            let result = toplevel
                .catch_signals()
                .handle_shutdown_requests(Duration::from_secs(5))
                .await;

            if let Err(e) = &result {
                tracing::error!("Application finished with an error (as expected): {}", e);
            }
        }
        "cancel" => {
            let on_subsystem_cancelled = {
                let hook_events = hook_events.clone();
                move |name: Arc<str>| {
                    let msg = format!("ROOT CANCEL HOOK: Root subsystem '{name}' was cancelled because Toplevel was dropped.");
                    eprintln!("{}", msg);
                    hook_events.lock().unwrap().push(msg);
                }
            };

            tracing::info!("Creating Toplevel...");
            let toplevel = Toplevel::new_with_hooks(
                async |s| {
                    s.start(SubsystemBuilder::new("Endless", endless_subsystem));
                    s.on_shutdown_requested().await
                },
                default_on_subsystem_error,
                on_subsystem_cancelled,
            );
            
            tracing::info!("Toplevel will be dropped now...");
            drop(toplevel);
            
            sleep(Duration::from_millis(100)).await;
            tracing::info!("Application finished.");
        }
        _ => {
            tracing::error!("Unknown scenario: {}", scenario);
        }
    }

    println!("\n-- Toplevel Hooks Report --");
    for event in hook_events.lock().unwrap().iter() {
        println!("- {}", event);
    }
    println!("---------------------------\n");

    Ok(())
}
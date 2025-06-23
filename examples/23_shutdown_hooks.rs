//! This example demonstrates how to use custom shutdown hooks to react to
//! different stages of the shutdown process.
//!
//! It provides a `MyShutdownHooks` struct that logs lifecycle events into a vector.
//! The program can be run in different modes to trigger various shutdown scenarios
//! and observe the corresponding hook calls.
//!
//! Run with: `cargo run --example 23_shutdown_hooks [SCENARIO]`
//!
//! Where `[SCENARIO]` can be one of:
//!   - `normal` (default): Demonstrates a normal shutdown triggered by Ctrl+C.
//!   - `fail`: Demonstrates a shutdown triggered by a subsystem error.
//!   - `timeout`: Demonstrates a shutdown that times out.
//!   - `finished`: Demonstrates the hook for when all subsystems finish without a shutdown signal.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    errors::SubsystemError, ErrTypeTraits, ShutdownHooks, SubsystemBuilder, SubsystemHandle,
    Toplevel,
};

#[derive(Clone)]
struct MyShutdownHooks {
    events: Arc<Mutex<Vec<String>>>,
}

impl MyShutdownHooks {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl ShutdownHooks for MyShutdownHooks {
    async fn on_subsystems_finished(&mut self) {
        let msg = "All subsystems finished.".to_string();
        tracing::info!("HOOK: {msg}");
        self.events.lock().unwrap().push(msg);
    }

    async fn on_shutdown_requested(&mut self) {
        let msg = "Shutdown requested.".to_string();
        tracing::info!("HOOK: {msg}");
        self.events.lock().unwrap().push(msg);
    }

    async fn on_shutdown_finished<ErrType: ErrTypeTraits>(
        &mut self,
        errors: &[SubsystemError<ErrType>],
    ) {
        let msg = if errors.is_empty() {
            "Shutdown finished successfully.".to_string()
        } else {
            let error_names: Vec<&str> = errors.iter().map(|e| e.name()).collect();
            format!("Shutdown finished with errors: {error_names:?}")
        };
        tracing::info!("HOOK: {msg}");
        self.events.lock().unwrap().push(msg);
    }

    async fn on_shutdown_timeout(&mut self) {
        let msg = "Shutdown timed out!".to_string();
        tracing::error!("HOOK: {msg}");
        self.events.lock().unwrap().push(msg);
    }
}

/// A subsystem that waits for a shutdown signal.
async fn subsys_normal(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem 'normal' started, waiting for shutdown signal...");
    subsys.on_shutdown_requested().await;
    tracing::info!("Subsystem 'normal' shutting down.");
    sleep(Duration::from_millis(100)).await;
    Ok(())
}

/// A subsystem that fails after a short delay.
async fn subsys_fail(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem 'fail' started, will fail in 500ms.");
    sleep(Duration::from_millis(500)).await;
    Err(anyhow!("Subsystem 'fail' failed as planned."))
}

/// A subsystem that takes too long to shut down.
async fn subsys_timeout(subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem 'timeout' started, waiting for shutdown signal...");
    subsys.on_shutdown_requested().await;
    tracing::info!("Subsystem 'timeout' shutting down, will take too long.");
    sleep(Duration::from_secs(2)).await;
    Ok(())
}

/// A subsystem that finishes on its own.
async fn subsys_finished(_subsys: SubsystemHandle) -> Result<()> {
    tracing::info!("Subsystem 'finished' started, will stop in 500ms.");
    sleep(Duration::from_millis(500)).await;
    tracing::info!("Subsystem 'finished' stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let scenario = std::env::args().nth(1).unwrap_or("normal".to_string());

    let hooks = MyShutdownHooks::new();
    let hooks_clone = hooks.clone();

    let toplevel = Toplevel::new(async move |s| match scenario.as_str() {
        "normal" => {
            s.start(SubsystemBuilder::new("Normal", subsys_normal));
        }
        "fail" => {
            s.start(SubsystemBuilder::new("Fail", subsys_fail));
            s.start(SubsystemBuilder::new("Normal", subsys_normal));
        }
        "timeout" => {
            s.start(SubsystemBuilder::new("Timeout", subsys_timeout));
        }
        "finished" => {
            s.start(SubsystemBuilder::new("Finished", subsys_finished));
        }
        _ => {
            tracing::error!("Unknown scenario: {}", scenario);
            s.request_shutdown();
        }
    });

    let result = toplevel
        .catch_signals()
        .handle_shutdown_requests_with_hooks(Duration::from_secs(1), hooks_clone)
        .await;

    println!("\n-- Shutdown Hooks Report --");
    for event in hooks.get_events() {
        println!("- {}", event);
    }
    println!("---------------------------\n");

    if let Err(e) = &result {
        tracing::error!("Application finished with an error: {}", e);
    } else {
        tracing::info!("Application finished successfully.");
    }

    result.map_err(Into::into)
}
//! This crate provides utility functions to perform a graceful shutdown on tokio-rs based services.
//!
//! Specifically, it provides:
//!
//! - Listening for shutdown requests from within subsystems
//! - Manual shutdown initiation from within subsystems
//! - Automatic shutdown on
//!     - SIGINT/SIGTERM/Ctrl+C
//!     - Subsystem failure
//!     - Subsystem panic
//! - Clean shutdown procedure with timeout and error propagation
//! - Subsystem nesting
//!
//! # Example
//!
//! This example shows an minimal example of how to launch an asynchronous subsystem with the help of this crate.
//!
//! The program will react to Ctrl-C/SIGINT/SIGTERM and will shut down gracefully and reliably.
//!
//! ```
//! use anyhow::Result;
//! use async_trait::async_trait;
//! use env_logger::{Builder, Env};
//! use log;
//! use tokio::time::{sleep, Duration};
//! use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle, Toplevel};
//!
//! struct MySubsystem {}
//!
//! #[async_trait]
//! impl AsyncSubsystem for MySubsystem {
//!     async fn run(&mut self, subsys: SubsystemHandle) -> Result<()> {
//!         log::info!("MySubsystem started.");
//!         subsys.on_shutdown_requested().await;
//!         log::info!("Shutting down MySubsystem ...");
//!         sleep(Duration::from_millis(500)).await;
//!         log::info!("MySubsystem stopped.");
//!         Ok(())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Init logging
//!     Builder::from_env(Env::default().default_filter_or("debug")).init();
//!
//!     // Create toplevel
//!     Toplevel::new()
//!         .start("MySubsystem", MySubsystem {})
//!         .catch_signals()
//!         .wait_for_shutdown(Duration::from_millis(1000))
//!         .await
//! }
//! ```
//!

mod exit_state;
mod runner;
mod shutdown_token;
mod signal_handling;
mod subsystem;
mod toplevel;

pub use shutdown_token::ShutdownToken;
pub use subsystem::{AsyncSubsystem, SubsystemHandle};
pub use toplevel::Toplevel;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

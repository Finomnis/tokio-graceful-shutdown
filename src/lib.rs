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
//! - Partial shutdown of a selected subsystem tree
//! - Customizable hooks for shutdown lifecycle events
//!
//! # Example
//!
//! This example shows a minimal example of how to launch an asynchronous subsystem with the help of this crate.
//!
//! It contains a countdown subsystem that will end the program after 10 seconds.
//! During the countdown, the program will react to Ctrl-C/SIGINT/SIGTERM and will cancel the countdown task accordingly.
//!
//! ```
//! use miette::Result;
//! use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
//! use tokio::time::{sleep, Duration};
//!
//! async fn countdown() {
//!     for i in (1..=5).rev() {
//!         tracing::info!("Shutting down in: {}", i);
//!         sleep(Duration::from_millis(1000)).await;
//!     }
//! }
//!
//! async fn countdown_subsystem(subsys: SubsystemHandle) -> Result<()> {
//!     tokio::select! {
//!         _ = subsys.on_shutdown_requested() => {
//!             tracing::info!("Countdown cancelled.");
//!         },
//!         _ = countdown() => {
//!             subsys.request_shutdown();
//!         }
//!     };
//!
//!     Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Init logging
//!     tracing_subscriber::fmt()
//!         .with_max_level(tracing::Level::TRACE)
//!         .init();
//!
//!     // Setup and execute subsystem tree
//!     Toplevel::new(async |s| {
//!         s.start(SubsystemBuilder::new("Countdown", countdown_subsystem));
//!     })
//!     .catch_signals()
//!     .handle_shutdown_requests(Duration::from_millis(1000))
//!     .await
//!     .map_err(Into::into)
//! }
//! ```
//!
//!
//! The [`Toplevel`] object represents the root object of the subsystem tree
//! and is the main entry point of how to interact with this crate.
//! Creating a [`Toplevel`] object initially spawns a simple subsystem, which can then
//! spawn further subsystems recursively.
//!
//! The [`catch_signals()`](Toplevel::catch_signals) method signals the `Toplevel` object to listen for SIGINT/SIGTERM/Ctrl+C and initiate a shutdown thereafter.
//!
//! [`handle_shutdown_requests()`](Toplevel::handle_shutdown_requests) is the final and most important method of `Toplevel`. It idles until the program enters the shutdown mode. Then, it collects all the return values of the subsystems, determines the global error state and makes sure the shutdown happens within the given timeout.
//! Lastly, it returns an error value that can be directly used as a return code for `main()`.
//!
//! Further, the way to register and start a new submodule is to provide
//! a submodule function/lambda to [`SubsystemHandle::start`].
//! If additional arguments shall to be provided to the submodule, it is necessary to create
//! a submodule `struct`. Further details can be seen in the `examples` directory of the repository.
//!
//! Finally, you can see the [`SubsystemHandle`] object that gets provided to the subsystem.
//! It is the main way of the subsystem to communicate with this crate.
//! It enables the subsystem to start nested subsystems, to react to shutdown requests or
//! to initiate a shutdown.
//!

#![deny(unreachable_pub)]
#![deny(missing_docs)]
#![doc(
    issue_tracker_base_url = "https://github.com/Finomnis/tokio-graceful-shutdown/issues",
    test(no_crate_inject, attr(deny(warnings))),
    test(attr(allow(dead_code)))
)]

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// A collection of traits a custom error has to fulfill in order to be
/// usable as the `ErrType` of [Toplevel].
pub trait ErrTypeTraits:
    std::fmt::Debug + std::fmt::Display + 'static + Send + Sync + Sized
{
}
impl<T> ErrTypeTraits for T where
    T: std::fmt::Debug + std::fmt::Display + 'static + Send + Sync + Sized
{
}

pub mod errors;

mod error_action;
mod future_ext;
mod into_subsystem;
mod runner;
mod shutdown_hooks;
mod signal_handling;
mod subsystem;
mod tokio_task;
mod toplevel;
mod utils;

pub use error_action::ErrorAction;
pub use future_ext::FutureExt;
pub use into_subsystem::IntoSubsystem;
pub use shutdown_hooks::{DefaultShutdownHooks, ShutdownHooks};
pub use signal_handling::{DefaultSignalHooks, SignalHooks};
pub use subsystem::NestedSubsystem;
pub use subsystem::SubsystemBuilder;
pub use subsystem::SubsystemFinishedFuture;
pub use subsystem::SubsystemHandle;
pub use toplevel::Toplevel;

use crate::errors::SubsystemError;
use std::sync::Arc;

/// The default error hook used by [`Toplevel::new`].
///
/// Logs uncaught subsystem errors and panics to `tracing::error!`.
pub fn default_on_subsystem_error<ErrType: ErrTypeTraits>(e: &SubsystemError<ErrType>) {
    match e {
        SubsystemError::Panicked(name) => {
            tracing::error!("Uncaught panic from subsystem '{name}'.")
        }
        SubsystemError::Failed(name, e) => {
            tracing::error!("Uncaught error from subsystem '{name}': {e}")
        }
    }
}

/// The default cancellation hook used by [`Toplevel::new`].
///
/// Logs a warning with `tracing::warn!` when a subsystem is cancelled.
pub fn default_on_subsystem_cancelled(name: Arc<str>) {
    tracing::warn!("Subsystem cancelled: '{name}'");
}

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
//! async fn countdown_subsystem(subsys: &mut SubsystemHandle) -> Result<()> {
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
//!     Toplevel::new(async |s: &mut SubsystemHandle| {
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

/// An async function that can be used as a subsystem.
///
/// Note: External users should not implement this trait directly.
/// Prefer passing `async fn` or async closures; this trait exists to
/// model those in the public API and may evolve.
pub trait AsyncSubsysFn<A, O>: Send + FnOnce(A) -> Self::Fut {
    /// The produced subsystem future
    type Fut: Future<Output = O> + Send;
}

// This trick allows us to generate a “FnOnce”-like bound with only one lifetime parameter,
// so that functions which capture the input argument’s lifetime in their output
// (i.e., async functions) can meet the bound.
impl<A, O, Out, F> AsyncSubsysFn<A, O> for F
where
    Out: Future<Output = O> + Send,
    F: Send + FnOnce(A) -> Out,
{
    type Fut = Out;
}
pub mod errors;

mod error_action;
mod future_ext;
mod into_subsystem;
mod runner;
mod signal_handling;
mod subsystem;
mod tokio_task;
mod toplevel;
mod utils;

pub use error_action::ErrorAction;
pub use future_ext::FutureExt;
pub use into_subsystem::IntoSubsystem;
pub use subsystem::NestedSubsystem;
pub use subsystem::SubsystemBuilder;
pub use subsystem::SubsystemFinishedFuture;
pub use subsystem::SubsystemHandle;
pub use toplevel::Toplevel;

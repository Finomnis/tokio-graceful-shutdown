use core::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use crate::{BoxedError, ErrTypeTraits, SubsystemHandle};

type SubsystemFuture<Err> = dyn Future<Output = Result<(), Err>> + Send + 'static;
type SubsystemFunction<Err, ErrWrapper> =
    dyn FnOnce(SubsystemHandle<ErrWrapper>) -> Pin<Box<SubsystemFuture<Err>>> + Send + 'static;

#[async_trait]
/// Allows a struct to be used as a subsystem.
///
/// Implementing this trait requires the `async_trait` dependency.
///
/// Using a struct that does not implement this trait as a subsystem is possible
/// by wrapping it in an async closure. This trait exists primarily
/// for convenience.
///
/// The template parameter of the trait is the error type
/// that the subsytem returns.
///
/// # Examples
///
/// ```
/// use miette::Result;
/// use tokio::time::Duration;
/// use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle, Toplevel};
///
/// struct MySubsystem;
///
/// #[async_trait::async_trait]
/// impl IntoSubsystem<miette::Report> for MySubsystem {
///     async fn run(self, subsys: SubsystemHandle) -> Result<()> {
///         subsys.initiate_shutdown();
///         Ok(())
///     }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create toplevel
///     Toplevel::new()
///         .start("Subsys1", MySubsystem{}.into_subsystem())
///         .catch_signals()
///         .handle_shutdown_requests(Duration::from_millis(500))
///         .await
///         .map_err(Into::into)
/// }
/// ```
///
pub trait IntoSubsystem<Err, ErrWrapper = BoxedError>
where
    Self: Sized + Send + Sync + 'static,
    Err: Into<ErrWrapper>,
    ErrWrapper: ErrTypeTraits,
{
    /// The logic of the subsystem.
    ///
    /// Will be called as soon as the subsystem gets started.
    ///
    /// Returning an error automatically initiates a shutdown.
    ///
    /// For more information about subsystem functions, see
    /// [`SubsystemHandle::start()`](crate::SubsystemHandle::start).
    async fn run(self, subsys: SubsystemHandle<ErrWrapper>) -> Result<(), Err>;

    /// Converts the object into a type that can be passed into
    /// [`SubsystemHandle::start()`](crate::SubsystemHandle::start).
    fn into_subsystem(self) -> Box<SubsystemFunction<Err, ErrWrapper>> {
        Box::new(|handle: SubsystemHandle<ErrWrapper>| {
            Box::pin(async move { self.run(handle).await })
        })
    }
}

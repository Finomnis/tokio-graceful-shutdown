use core::future::Future;
use std::pin::Pin;

use crate::{BoxedError, ErrTypeTraits, SubsystemHandle};

/// Allows a struct to be used as a subsystem.
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
/// use tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel};
///
/// struct MySubsystem;
///
/// impl IntoSubsystem<miette::Report> for MySubsystem {
///     async fn run(self, subsys: SubsystemHandle) -> Result<()> {
///         subsys.request_shutdown();
///         Ok(())
///     }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create toplevel
///     Toplevel::new(|s| async move {
///         s.start(SubsystemBuilder::new(
///             "Subsys1", MySubsystem{}.into_subsystem()
///         ));
///     })
///     .catch_signals()
///     .handle_shutdown_requests(Duration::from_millis(500))
///     .await
///     .map_err(Into::into)
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
    fn run(
        self,
        subsys: SubsystemHandle<ErrWrapper>,
    ) -> impl std::future::Future<Output = Result<(), Err>> + Send;

    /// Converts the object into a type that can be passed into
    /// [`SubsystemHandle::start()`](crate::SubsystemHandle::start).
    fn into_subsystem(
        self,
    ) -> impl FnOnce(SubsystemHandle<ErrWrapper>) -> Pin<Box<dyn Future<Output = Result<(), Err>> + Send>>
    {
        |handle: SubsystemHandle<ErrWrapper>| Box::pin(async move { self.run(handle).await })
    }
}

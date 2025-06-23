use async_trait::async_trait;
use crate::errors::SubsystemError;
use crate::ErrTypeTraits;

#[async_trait]
/// A trait that allows executing custom logic at various points of the shutdown lifecycle.
/// 
/// Implementing this trait requires the `async_trait` dependency.
///
/// It can be passed to [`Toplevel::handle_shutdown_requests_with_hooks`](crate::Toplevel::handle_shutdown_requests_with_hooks).
///
/// All methods have a default implementation that logs the event, so you only need to
/// implement the ones you are interested in.
pub trait ShutdownHooks: Send {
    /// Called when all subsystems have finished execution without any particular shutdown being 
    /// requested.
    async fn on_subsystems_finished(&mut self) {
        tracing::info!("All subsystems finished.");
    }

    /// Called when a shutdown is requested, either through a signal or by calling
    /// [`SubsystemHandle::request_shutdown`](crate::SubsystemHandle::request_shutdown).
    async fn on_shutdown_requested(&mut self) {
        tracing::info!("Shutting down ...");
    }

    /// Called after a requested shutdown has completed successfully within the given timeout.
    ///
    /// It receives a slice of all subsystem errors that occurred.
    async fn on_shutdown_finished<ErrType: ErrTypeTraits>(
        &mut self,
        errors: &[SubsystemError<ErrType>],
    ) {
        if errors.is_empty() {
            tracing::info!("Shutdown finished.");
        } else {
            tracing::warn!("Shutdown finished with errors.");
        }
    }

    /// Called when a requested shutdown does not complete within the given timeout.
    async fn on_shutdown_timeout(&mut self) {
        tracing::error!("Shutdown timed out!");
    }
}

/// A default implementation of [`ShutdownHooks`] that provides logging for shutdown events.
///
/// This is used by [`Toplevel::handle_shutdown_requests`](crate::Toplevel::handle_shutdown_requests).
pub struct DefaultShutdownHooks;

impl ShutdownHooks for DefaultShutdownHooks {}

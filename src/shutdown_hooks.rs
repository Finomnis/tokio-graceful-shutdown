use async_trait::async_trait;
use crate::errors::SubsystemError;
use crate::ErrTypeTraits;

#[async_trait]
pub trait ShutdownHooks: Send {
    async fn on_subsystems_finished(&mut self) {
        tracing::info!("All subsystems finished.");
    }

    async fn on_shutdown_requested(&mut self) {
        tracing::info!("Shutting down ...");
    }

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

    async fn on_shutdown_timeout(&mut self) {
        tracing::error!("Shutdown timed out!");
    }
}

pub struct DefaultShutdownHooks;

impl ShutdownHooks for DefaultShutdownHooks {}

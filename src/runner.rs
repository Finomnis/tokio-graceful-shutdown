use crate::{
    errors::{SubsystemError, SubsystemFailure},
    ErrTypeTraits, ShutdownToken,
};
use std::future::Future;
use tokio::task::{JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;

pub struct SubsystemRunner<ErrType: ErrTypeTraits> {
    outer_joinhandle: JoinHandle<Result<(), SubsystemError<ErrType>>>,
    cancellation_token: CancellationToken,
}

/// Dropping the SubsystemRunner cancels the task.
///
/// In consequence, this means that dropping the Toplevel object cancels all tasks.
impl<ErrType: ErrTypeTraits> Drop for SubsystemRunner<ErrType> {
    fn drop(&mut self) {
        self.abort();
    }
}

impl<ErrType: ErrTypeTraits> SubsystemRunner<ErrType> {
    async fn handle_subsystem(
        mut inner_joinhandle: JoinHandle<Result<(), ErrType>>,
        shutdown_token: ShutdownToken,
        local_shutdown_token: ShutdownToken,
        name: String,
        cancellation_token: CancellationToken,
    ) -> Result<(), SubsystemError<ErrType>> {
        /// Maps the complicated return value of the subsystem joinhandle to an appropriate error
        fn map_subsystem_result<ErrType: ErrTypeTraits>(
            name: &str,
            result: Result<Result<(), ErrType>, JoinError>,
        ) -> Result<(), SubsystemError<ErrType>> {
            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(SubsystemError::Failed(
                    name.to_string(),
                    SubsystemFailure(e),
                )),
                Err(e) => Err(if e.is_cancelled() {
                    SubsystemError::Cancelled(name.to_string())
                } else {
                    SubsystemError::Panicked(name.to_string())
                }),
            }
        }

        let joinhandle_ref = &mut inner_joinhandle;
        let result = tokio::select! {
            result = joinhandle_ref => {
                map_subsystem_result(&name, result)
            },
            _ = cancellation_token.cancelled() => {
                inner_joinhandle.abort();
                map_subsystem_result(&name, inner_joinhandle.await)
            }
        };

        match &result {
            Ok(()) | Err(SubsystemError::Cancelled(_)) => {}
            Err(SubsystemError::Failed(name, e)) => {
                log::error!("Error in subsystem '{}': {:?}", name, e);
                if !local_shutdown_token.is_shutting_down() {
                    shutdown_token.shutdown();
                }
            }
            Err(SubsystemError::Panicked(name)) => {
                log::error!("Subsystem '{}' panicked", name);
                if !local_shutdown_token.is_shutting_down() {
                    shutdown_token.shutdown();
                }
            }
        };

        result
    }

    pub fn new<Fut: 'static + Future<Output = Result<(), ErrType>> + Send>(
        name: String,
        shutdown_token: ShutdownToken,
        local_shutdown_token: ShutdownToken,
        cancellation_token: CancellationToken,
        subsystem_future: Fut,
    ) -> Self {
        // Spawn to nested tasks.
        // This enables us to catch panics, as panics get returned through a JoinHandle.
        let inner_joinhandle = tokio::spawn(subsystem_future);
        let outer_joinhandle = tokio::spawn(Self::handle_subsystem(
            inner_joinhandle,
            shutdown_token,
            local_shutdown_token,
            name,
            cancellation_token.clone(),
        ));

        Self {
            outer_joinhandle,
            cancellation_token,
        }
    }

    pub async fn join(&mut self) -> Result<(), SubsystemError<ErrType>> {
        // Safety: we are in full control over the outer_joinhandle and the
        // code it runs. Therefore, if this either returns a panic or a cancelled,
        // it's a programming error on our side.
        // Therefore using unwrap() here is the correct way of handling it.
        // (this and the fact that unreachable code would decrease our test coverage)
        (&mut self.outer_joinhandle).await.unwrap()
    }

    pub fn abort(&self) {
        self.cancellation_token.cancel();
    }
}

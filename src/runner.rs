use crate::{
    errors::SubsystemError,
    event::{Event, EventTrigger},
    BoxedError, ShutdownToken,
};
use std::future::Future;
use tokio::task::{JoinError, JoinHandle};

pub struct SubsystemRunner {
    name: String,
    outer_joinhandle: JoinHandle<Result<(), SubsystemError>>,
    request_cancellation: EventTrigger,
}

/// Dropping the SubsystemRunner cancels the task.
///
/// In consequence, this means that dropping the Toplevel object cancels all tasks.
impl Drop for SubsystemRunner {
    fn drop(&mut self) {
        self.abort();
    }
}

impl SubsystemRunner {
    async fn handle_subsystem(
        mut inner_joinhandle: JoinHandle<Result<(), BoxedError>>,
        shutdown_token: ShutdownToken,
        local_shutdown_token: ShutdownToken,
        name: String,
        cancellation_requested: Event,
    ) -> Result<(), SubsystemError> {
        /// Maps the complicated return value of the subsystem joinhandle to an appropriate error
        fn map_subsystem_result(
            name: &str,
            result: Result<Result<(), BoxedError>, JoinError>,
        ) -> Result<(), SubsystemError> {
            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(SubsystemError::Failed(name.to_string(), e)),
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
            _ = cancellation_requested.wait() => {
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

    pub fn new<Fut: 'static + Future<Output = Result<(), BoxedError>> + Send>(
        name: String,
        shutdown_token: ShutdownToken,
        local_shutdown_token: ShutdownToken,
        subsystem_future: Fut,
    ) -> Self {
        let (cancellation_requested, request_cancellation) = Event::create();

        // Spawn to nested tasks.
        // This enables us to catch panics, as panics get returned through a JoinHandle.
        let inner_joinhandle = tokio::spawn(subsystem_future);
        let outer_joinhandle = tokio::spawn(Self::handle_subsystem(
            inner_joinhandle,
            shutdown_token,
            local_shutdown_token,
            name.clone(),
            cancellation_requested,
        ));

        Self {
            name,
            outer_joinhandle,
            request_cancellation,
        }
    }

    pub async fn join(&mut self) -> Result<(), SubsystemError> {
        match (&mut self.outer_joinhandle).await {
            Ok(result) => result,
            Err(e) => Err(if e.is_cancelled() {
                SubsystemError::Cancelled(self.name.clone())
            } else {
                SubsystemError::Panicked(self.name.clone())
            }),
        }
    }

    pub fn abort(&self) {
        self.request_cancellation.set();
    }
}

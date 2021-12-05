use crate::{
    event::{Event, EventTrigger},
    ShutdownToken,
};
use anyhow::Result;
use std::future::Future;
use tokio::task::{JoinError, JoinHandle};

pub struct SubsystemRunner {
    outer_joinhandle: JoinHandle<Result<Result<(), ()>, JoinError>>,
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
        mut inner_joinhandle: JoinHandle<Result<()>>,
        shutdown_token: ShutdownToken,
        local_shutdown_token: ShutdownToken,
        name: String,
        cancellation_requested: Event,
    ) -> Result<Result<(), ()>, JoinError> {
        let joinhandle_ref = &mut inner_joinhandle;
        tokio::select! {
            result = joinhandle_ref => {
                    match result {
                        Ok(Ok(())) => {Ok(Ok(()))},
                        Ok(Err(e)) => {
                            log::error!("Error in subsystem '{}': {:?}", name, e);
                            if !local_shutdown_token.is_shutting_down() {
                                shutdown_token.shutdown();
                            }
                            Ok(Err(()))
                        },
                        Err(e) => {
                            log::error!("Error in subsystem '{}': {}", name, e);
                            if !local_shutdown_token.is_shutting_down() {
                                shutdown_token.shutdown();
                            }
                            Err(e)
                        }
                    }
            },
            _ = cancellation_requested.wait() => {
                inner_joinhandle.abort();
                match inner_joinhandle.await {
                    Ok(Ok(())) => Ok(Ok(())),
                    Ok(Err(e)) => {
                        log::error!("Error in subsystem '{}': {:?}", name, e);
                        Ok(Err(()))
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    pub fn new<Fut: 'static + Future<Output = Result<()>> + Send>(
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
            name,
            cancellation_requested,
        ));

        Self {
            outer_joinhandle,
            request_cancellation,
        }
    }

    pub async fn join(&mut self) -> Result<Result<(), ()>, JoinError> {
        (&mut self.outer_joinhandle).await?
    }

    pub fn abort(&self) {
        self.request_cancellation.set();
    }
}

use crate::{
    event::{Event, EventTrigger},
    ShutdownToken, SubsystemHandle,
};
use anyhow::Result;
use std::{future::Future, slice::Join};
use tokio::task::{JoinError, JoinHandle};

pub struct SubsystemRunner {
    shutdown_token: ShutdownToken,
    outer_joinhandle: JoinHandle<Result<Result<(), ()>, JoinError>>,
    request_cancellation: EventTrigger,
}

impl SubsystemRunner {
    async fn handle_subsystem(
        mut inner_joinhandle: JoinHandle<Result<()>>,
        shutdown_token: ShutdownToken,
        name: String,
        cancellation_requested: Event,
    ) -> Result<Result<(), ()>, JoinError> {
        let inner_joinhandle_ref = &mut inner_joinhandle;
        let result = tokio::select! {
            result = inner_joinhandle_ref => {
                match result {
                    Ok(Ok(())) => {Ok(Ok(()))},
                    Ok(Err(e)) => {
                        log::error!("Error in subsystem '{}': {:?}", name, e);
                        shutdown_token.shutdown();
                        Ok(Err(()))
                    },
                    Err(e) => {
                        log::error!("Error in subsystem '{}': {:?}", name, e);
                        shutdown_token.shutdown();
                        Err(e)
                    }
                }
            },
            _ = cancellation_requested.wait() => {
                // If cancellation is requested, cancel the subsystem and query its return
                // value
                inner_joinhandle_ref.abort();
                match inner_joinhandle_ref.await {
                    Ok(Ok(())) => {Ok(Ok(()))},
                    Ok(Err(e)) => {
                        log::error!("Error in subsystem '{}': {:?}", name, e);
                        Ok(Err(()))
                    },
                    Err(e) => {
                        Err(e)
                    }
                }
            }
        };

        result
    }

    pub fn new<Fut: 'static + Future<Output = Result<()>> + Send>(
        name: String,
        shutdown_token: ShutdownToken,
        subsystem_future: Fut,
    ) -> Self {
        let (cancellation_requested, request_cancellation) = Event::create();

        // Spawn to nested tasks.
        // This enables us to catch panics, as panics get returned through a JoinHandle.
        let mut inner_joinhandle = tokio::spawn(subsystem_future);
        let outer_joinhandle = tokio::spawn(Self::handle_subsystem(
            inner_joinhandle,
            shutdown_token.clone(),
            name,
            cancellation_requested,
        ));

        Self {
            shutdown_token,
            outer_joinhandle,
            request_cancellation: request_cancellation,
        }
    }

    pub async fn join(&mut self) -> Result<Result<(), ()>, JoinError> {
        (&mut self.outer_joinhandle).await?
    }

    pub fn abort(&self) {
        self.request_cancellation.set();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shutdown_token::create_shutdown_token;
    use crate::subsystem::SubsystemData;

    use anyhow::{anyhow, Result};
    use std::sync::Arc;
    use tokio::sync::oneshot;
    use tokio::time::{sleep, Duration};

    struct TriggerableSubsystem {
        receiver: oneshot::Receiver<Result<()>>,
    }

    impl TriggerableSubsystem {
        async fn run(self, subsys: SubsystemHandle) -> Result<()> {
            tokio::select! {
                _ = subsys.on_shutdown_requested() => Err(anyhow!("Cancelled!")),
                e = self.receiver => e?
            }
        }
    }

    #[tokio::test]
    async fn forwards_subsystem_handle_to_runner() {
        // Arrange
        let shutdown_token = create_shutdown_token();
        let subsys_data = Arc::new(SubsystemData::new("dummy", shutdown_token.clone()));
        let subsys_handle = SubsystemHandle::new(subsys_data);
        let (trigger, receiver) = oneshot::channel();
        let subsys = TriggerableSubsystem { receiver };

        // Act
        let runner = run_subsystem("dummy".into(), |a| subsys.run(a), subsys_handle);
        let actor = async {
            sleep(Duration::from_millis(100)).await;
            shutdown_token.shutdown();
            sleep(Duration::from_millis(100)).await;

            // Assert
            let result = trigger.send(Ok(()));
            if let Err(Ok(())) = result {
            } else {
                panic!("Expected trigger.send to fail, as the other side should be closed by now!");
            }
        };
        let (result, ()) = tokio::join!(runner, actor);

        // Assert
        assert!(shutdown_token.is_shutting_down());
        assert_eq!(result, Err(()));
    }

    #[tokio::test]
    async fn returncode_error_causes_shutdown() {
        // Arrange
        let shutdown_token = create_shutdown_token();
        let subsys_data = Arc::new(SubsystemData::new("dummy", shutdown_token.clone()));
        let subsys_handle = SubsystemHandle::new(subsys_data);
        let (trigger, receiver) = oneshot::channel();
        let subsys = TriggerableSubsystem { receiver };

        // Act
        trigger.send(Err(anyhow!("foobar"))).unwrap();
        let result = run_subsystem("dummy".into(), |a| subsys.run(a), subsys_handle).await;

        // Assert
        assert!(shutdown_token.is_shutting_down());
        assert_eq!(result, Err(()));
    }

    #[tokio::test]
    async fn returncode_success_causes_no_shutdown() {
        // Arrange
        let shutdown_token = create_shutdown_token();
        let subsys_data = Arc::new(SubsystemData::new("dummy", shutdown_token.clone()));
        let subsys_handle = SubsystemHandle::new(subsys_data);
        let (trigger, receiver) = oneshot::channel();
        let subsys = TriggerableSubsystem { receiver };

        // Act
        trigger.send(Ok(())).unwrap();
        let result = run_subsystem("dummy".into(), |a| subsys.run(a), subsys_handle).await;

        // Assert
        assert!(!shutdown_token.is_shutting_down());
        assert_eq!(result, Ok(()));
    }
}

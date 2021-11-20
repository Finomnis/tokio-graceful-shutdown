use crate::SubsystemHandle;
use anyhow::Result;
use std::future::Future;

pub async fn run_subsystem<
    Fut: Future<Output = Result<()>> + Send,
    S: FnOnce(SubsystemHandle) -> Fut + Send,
>(
    name: String,
    subsystem: S,
    subsystem_handle: SubsystemHandle,
) -> Result<(), ()> {
    let shutdown_token = subsystem_handle.shutdown_token().clone();

    let result = subsystem(subsystem_handle).await;
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            log::error!("Error in subsystem '{}': {:?}", name, e);
            shutdown_token.shutdown();
            Err(())
        }
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

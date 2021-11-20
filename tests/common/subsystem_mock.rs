use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::sync::oneshot;
use tokio_graceful_shutdown::{AsyncSubsystem, SubsystemHandle};

pub struct SubsystemMock {
    return_value: oneshot::Receiver<Result<()>>,
    started: oneshot::Sender<()>,
    stopped: oneshot::Sender<()>,
}
pub struct SubsystemMockController {
    pub return_value: oneshot::Sender<Result<()>>,
    pub started: oneshot::Receiver<()>,
    pub stopped: oneshot::Receiver<()>,
}

#[async_trait]
impl AsyncSubsystem for SubsystemMock {
    async fn run(mut self, subsys: SubsystemHandle) -> Result<()> {
        self.started.send(()).unwrap();
        let result = tokio::select! {
            _ = subsys.on_shutdown_requested() => Err(anyhow!("Shut down")),
            e = self.return_value => e?
        };
        self.stopped.send(()).unwrap();
        result
    }
}

pub fn create_subsystem_mock() -> (SubsystemMock, SubsystemMockController) {
    let (return_value_tx, return_value_rx) = oneshot::channel();
    let (started_tx, started_rx) = oneshot::channel();
    let (stopped_tx, stopped_rx) = oneshot::channel();
    (
        SubsystemMock {
            return_value: return_value_rx,
            started: started_tx,
            stopped: stopped_tx,
        },
        SubsystemMockController {
            return_value: return_value_tx,
            started: started_rx,
            stopped: stopped_rx,
        },
    )
}

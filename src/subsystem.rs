use std::sync::Arc;

use anyhow::Result;
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::join;
use futures::future::join_all;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::runner::run_subsystem;
use crate::shutdown_token::ShutdownToken;

pub struct SubsystemData {
    name: String,
    subsystems: RwLock<Option<SubsystemDescriptors>>,
    shutdown_token: ShutdownToken,
}

pub struct SubsystemHandle {
    shutdown_token: ShutdownToken,
    data: Arc<SubsystemData>,
}

struct SubsystemDescriptors {
    data: Vec<Arc<SubsystemData>>,
    joinhandles: Vec<JoinHandle<Result<(), ()>>>,
}

impl SubsystemData {
    pub fn new(name: &str, shutdown_token: ShutdownToken) -> Self {
        Self {
            name: name.to_string(),
            subsystems: RwLock::new(Some(SubsystemDescriptors {
                data: Vec::new(),
                joinhandles: Vec::new(),
            })),
            shutdown_token,
        }
    }

    pub async fn add_subsystem(
        &self,
        subsystem: Arc<SubsystemData>,
        joinhandle: JoinHandle<Result<(), ()>>,
    ) {
        match self.subsystems.write().await.as_mut() {
            Some(subsystems) => {
                subsystems.joinhandles.push(joinhandle);
                subsystems.data.push(subsystem);
            }
            None => {
                log::error!("Unable to add subsystem, system already shutting down!");
                joinhandle.abort();
            }
        }
    }

    #[async_recursion]
    pub async fn perform_shutdown(&self) -> Result<()> {
        let mut subsystems_guard = self.subsystems.write().await;
        let subsystems = subsystems_guard.as_mut().take().ok_or(anyhow::anyhow!(
            "Unknown error, attempted to wait for subprocesses twice! Should never happen."
        ))?;

        let joinhandles_finished = join_all(subsystems.joinhandles.iter_mut());
        let subsystems_finished = join_all(
            subsystems
                .data
                .iter_mut()
                .map(|data| data.perform_shutdown()),
        );

        match join(
            async {
                let joinhandles_finished = joinhandles_finished.await;

                let join_results = joinhandles_finished.iter().map(|e| match e {
                    Ok(Ok(())) => Ok("OK".to_string()),
                    Ok(Err(())) => Err("Failed".to_string()),
                    Err(e) => Err(format!("Internal error: {}", e)),
                });

                let mut result: Result<(), ()> = Ok(());
                for join_result in join_results {
                    let msg = match &join_result {
                        Ok(msg) => msg,
                        Err(msg) => msg,
                    };
                    log::debug!("Shutdown 'unknown': {}", msg);

                    let outcome = match join_result {
                        Ok(_) => Ok(()),
                        Err(_) => Err(()),
                    };
                    result = result.and(outcome);
                }

                result
            },
            async {
                match subsystems_finished
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>>>()
                {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            },
        )
        .await
        {
            (Ok(()), Ok(())) => Ok(()),
            (Err(()), Ok(())) => Err(anyhow::anyhow!("Shutdown failed.")),
            (_, Err(e)) => Err(e),
        }
    }
}

impl SubsystemHandle {
    pub fn new(data: Arc<SubsystemData>) -> Self {
        Self {
            shutdown_token: data.shutdown_token.clone(),
            data,
        }
    }

    pub async fn start<S: AsyncSubsystem + 'static + Send>(
        &mut self,
        name: &'static str,
        subsystem: S,
    ) -> &mut Self {
        let shutdown_token = self.shutdown_token.clone();

        let name = self.data.name.clone() + "/" + name;

        // Create subsystem data structure
        let new_subsystem = Arc::new(SubsystemData::new(&name, shutdown_token.clone()));

        // Create handle
        let subsystem_handle = SubsystemHandle::new(new_subsystem.clone());

        // Spawn new task
        let join_handle = tokio::spawn(run_subsystem(name, subsystem, subsystem_handle));

        // Store subsystem data
        self.data.add_subsystem(new_subsystem, join_handle).await;

        self
    }

    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown_token.clone()
    }

    pub async fn on_shutdown_requested(&self) {
        self.shutdown_token.wait_for_shutdown().await
    }
}

#[async_trait]
pub trait AsyncSubsystem {
    async fn run(&mut self, inst: SubsystemHandle) -> Result<()>;
}

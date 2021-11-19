use std::sync::Arc;

use anyhow::Result;
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::join;
use futures::future::join_all;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::exit_state::join_shutdown_results;
use crate::exit_state::ShutdownResults;
use crate::exit_state::SubprocessExitState;
use crate::runner::run_subsystem;
use crate::shutdown_token::ShutdownToken;

pub struct SubsystemData {
    name: String,
    subsystems: RwLock<Option<Vec<SubsystemDescriptor>>>,
    shutdown_token: ShutdownToken,
}

pub struct SubsystemHandle {
    shutdown_token: ShutdownToken,
    data: Arc<SubsystemData>,
}

struct SubsystemDescriptor {
    data: Arc<SubsystemData>,
    joinhandle: JoinHandle<Result<(), ()>>,
}

impl SubsystemData {
    pub fn new(name: &str, shutdown_token: ShutdownToken) -> Self {
        Self {
            name: name.to_string(),
            subsystems: RwLock::new(Some(Vec::new())),
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
                subsystems.push(SubsystemDescriptor {
                    joinhandle,
                    data: subsystem,
                });
            }
            None => {
                log::error!("Unable to add subsystem, system already shutting down!");
                joinhandle.abort();
            }
        }
    }

    #[async_recursion]
    pub async fn perform_shutdown(&self) -> ShutdownResults {
        let mut subsystems_guard = self.subsystems.write().await;
        let subsystems = match subsystems_guard.as_mut().take() {
            Some(a) => a,
            None => {
                panic!(
                    "Unknown error, attempted to wait for subprocesses twice! Should never happen."
                );
            }
        };

        let mut joinhandles = vec![];
        let mut subsystem_data = vec![];
        for SubsystemDescriptor { joinhandle, data } in subsystems {
            joinhandles.push((data.name.clone(), joinhandle));
            subsystem_data.push(data);
        }
        let joinhandles_finished = join_all(
            joinhandles
                .iter_mut()
                .map(|(name, joinhandle)| async { (name, joinhandle.await) }),
        );
        let subsystems_finished = join_all(
            subsystem_data
                .iter_mut()
                .map(|data| data.perform_shutdown()),
        );

        let (results_direct, results_recursive) = join(
            async {
                let joinhandles_finished = joinhandles_finished.await;

                let join_results = joinhandles_finished
                    .iter()
                    .map(|(name, result)| match result {
                        Ok(Ok(())) => Ok((name, "OK".to_string())),
                        Ok(Err(())) => Err((name, "Failed".to_string())),
                        Err(e) => Err((name, format!("Internal error: {}", e))),
                    })
                    .collect::<Vec<_>>();

                let exit_states = join_results
                    .iter()
                    .map(|e| {
                        let (name, msg) = match e {
                            Ok(msg) => msg,
                            Err(msg) => msg,
                        };
                        SubprocessExitState::new(name, &msg)
                    })
                    .collect::<Vec<_>>();

                match join_results.into_iter().collect::<Result<Vec<_>, _>>() {
                    Ok(_) => Ok(exit_states),
                    Err(_) => Err(exit_states),
                }
            },
            subsystems_finished,
        )
        .await;

        join_shutdown_results(results_direct, results_recursive)
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

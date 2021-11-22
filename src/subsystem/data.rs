use std::sync::Arc;
use tokio::sync::MutexGuard;

use anyhow::Result;
use async_recursion::async_recursion;
use futures::future::join;
use futures::future::join_all;
use std::sync::Mutex;

use super::SubsystemData;
use super::SubsystemDescriptor;
use crate::exit_state::{join_shutdown_results, ShutdownResults, SubprocessExitState};
use crate::runner::SubsystemRunner;
use crate::shutdown_token::ShutdownToken;

impl SubsystemData {
    pub fn new(name: &str, shutdown_token: ShutdownToken) -> Self {
        Self {
            name: name.to_string(),
            subsystems: Mutex::new(Some(Vec::new())),
            shutdown_token,
            shutdown_subsystems: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Registers a new subsystem in self.subsystems.
    ///
    /// If a shutdown is already running, self.subsystems will be 'None',
    /// and the newly spawned subsystem will be cancelled.
    pub fn add_subsystem(&self, subsystem: Arc<SubsystemData>, subsystem_runner: SubsystemRunner) {
        match self.subsystems.lock().unwrap().as_mut() {
            Some(subsystems) => {
                subsystems.push(SubsystemDescriptor {
                    subsystem_runner,
                    data: subsystem,
                });
            }
            None => {
                log::error!("Unable to add subsystem, system already shutting down!");
                subsystem_runner.abort();
            }
        }
    }

    /// Moves all subsystem descriptors to the self.shutdown_subsystem vector.
    /// This indicates to the subsystem that it should no longer be possible to
    /// spawn new nested subsystems.
    ///
    /// This is achieved by writing 'None' to self.subsystems.
    ///
    /// Preventing new nested subsystems to be registered is important to avoid
    /// a race condition where the subsystem could spawn a nested subsystem by calling
    /// [`SubsystemHandle.start`] during cleanup, leaking the new nested subsystem.
    ///
    /// (The place where adding new subsystems will fail is in [`SubsystemData.add_subsystem`])
    async fn prepare_shutdown(&self) -> MutexGuard<'_, Vec<SubsystemDescriptor>> {
        let mut shutdown_subsystems = self.shutdown_subsystems.lock().await;
        let mut subsystems = self.subsystems.lock().unwrap();
        if let Some(e) = subsystems.take() {
            shutdown_subsystems.extend(e.into_iter())
        };
        shutdown_subsystems
    }

    /// Recursively goes through all subsystems, awaits their join handles,
    /// and collects their exit states.
    ///
    /// Returns the collected subsystem exit states.
    ///
    /// This function can handle cancellation.
    #[async_recursion]
    pub async fn perform_shutdown(&self) -> ShutdownResults {
        let mut subsystems = self.prepare_shutdown().await;

        let mut subsystem_runners = vec![];
        let mut subsystem_data = vec![];
        for SubsystemDescriptor {
            subsystem_runner,
            data,
        } in subsystems.iter_mut()
        {
            subsystem_runners.push((data.name.clone(), subsystem_runner));
            subsystem_data.push(data);
        }
        let joinhandles_finished = join_all(
            subsystem_runners
                .iter_mut()
                .map(|(name, subsystem_runner)| async { (name, subsystem_runner.join().await) }),
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
                        SubprocessExitState::new(name, msg)
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

    #[async_recursion]
    pub async fn cancel_all_subsystems(&self) {
        let subsystems = self.prepare_shutdown().await;
        for subsystem in subsystems.iter() {
            subsystem.subsystem_runner.abort();
            subsystem.data.cancel_all_subsystems().await;
        }
    }
}

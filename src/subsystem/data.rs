use std::sync::Arc;
use tokio::sync::MutexGuard;

use async_recursion::async_recursion;
use futures::future::join;
use futures::future::join_all;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::NestedSubsystem;
use super::PartialShutdownError;
use super::SubsystemData;
use super::SubsystemDescriptor;
use super::SubsystemIdentifier;
use crate::exit_state::prettify_exit_states;
use crate::exit_state::{join_shutdown_results, ShutdownResults, SubprocessExitState};
use crate::runner::SubsystemRunner;
use crate::shutdown_token::ShutdownToken;
use crate::SubsystemError;

impl SubsystemData {
    pub fn new(
        name: &str,
        global_shutdown_token: ShutdownToken,
        local_shutdown_token: ShutdownToken,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            name: name.to_string(),
            subsystems: Mutex::new(Some(Vec::new())),
            global_shutdown_token,
            local_shutdown_token,
            cancellation_token,
            shutdown_subsystems: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Registers a new subsystem in self.subsystems.
    ///
    /// If a shutdown is already running, self.subsystems will be 'None',
    /// and the newly spawned subsystem will be cancelled.
    pub fn add_subsystem(
        &self,
        subsystem: Arc<SubsystemData>,
        subsystem_runner: SubsystemRunner,
    ) -> SubsystemIdentifier {
        let id = SubsystemIdentifier::create();
        match self.subsystems.lock().unwrap().as_mut() {
            Some(subsystems) => {
                subsystems.push(SubsystemDescriptor {
                    id: id.clone(),
                    subsystem_runner,
                    data: subsystem,
                });
            }
            None => {
                log::error!("Unable to add subsystem, parent subsystem already shutting down!");
                subsystem_runner.abort();
            }
        }
        id
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

    /// Recursively goes through all given subsystems, awaits their join handles,
    /// and collects their exit states.
    ///
    /// Returns the collected subsystem exit states.
    ///
    /// This function can handle cancellation.
    #[async_recursion]
    async fn perform_shutdown_on_subsystems(
        subsystems: &mut [SubsystemDescriptor],
    ) -> ShutdownResults {
        let mut subsystem_runners = vec![];
        let mut subsystem_data = vec![];
        for SubsystemDescriptor {
            id: _,
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
                .map(
                    |(name, subsystem_runner)| async move { (name, subsystem_runner.join().await) },
                ),
        );
        let subsystems_finished = join_all(
            subsystem_data
                .iter_mut()
                .map(|data| data.perform_shutdown()),
        );

        let (results_direct, results_recursive) = join(
            async {
                let joinhandles_finished = joinhandles_finished.await;

                joinhandles_finished
                    .into_iter()
                    .map(|(name, result)| {
                        SubprocessExitState::new(
                            name,
                            match &result {
                                Ok(()) => "OK",
                                Err(SubsystemError::Cancelled(_)) => "Cancelled",
                                Err(SubsystemError::Failed(_, _)) => "Failed",
                                Err(SubsystemError::Panicked(_)) => "Panicked",
                            },
                            result,
                        )
                    })
                    .collect()
            },
            subsystems_finished,
        )
        .await;

        join_shutdown_results(results_direct, results_recursive)
    }

    /// Recursively goes through all subsystems, awaits their join handles,
    /// and collects their exit states.
    ///
    /// Returns the collected subsystem exit states.
    ///
    /// This function can handle cancellation.
    pub async fn perform_shutdown(&self) -> ShutdownResults {
        let mut subsystems = self.prepare_shutdown().await;

        SubsystemData::perform_shutdown_on_subsystems(&mut subsystems).await
    }

    pub fn cancel_all_subsystems(&self) {
        self.cancellation_token.cancel();
    }

    pub async fn perform_partial_shutdown(
        &self,
        subsystem_handle: NestedSubsystem,
    ) -> Result<(), PartialShutdownError> {
        let subsystem = {
            let mut subsystems_mutex = self.subsystems.lock().unwrap();
            let subsystems = subsystems_mutex
                .as_mut()
                .ok_or(PartialShutdownError::AlreadyShuttingDown)?;
            let position = subsystems
                .iter()
                .position(|elem| elem.id == subsystem_handle.id)
                .ok_or(PartialShutdownError::SubsystemNotFound)?;
            subsystems.swap_remove(position)
        };

        // Initiate shutdown
        subsystem.data.local_shutdown_token.shutdown();

        // Wait for shutdown to finish
        let mut subsystem_vec = vec![subsystem];
        let exit_states = SubsystemData::perform_shutdown_on_subsystems(&mut subsystem_vec).await;

        // Prettify exit states
        let formatted_exit_states = prettify_exit_states(&exit_states);

        // Collect failed subsystems
        let failed_subsystems = exit_states
            .into_iter()
            .filter_map(|exit_state| match exit_state.raw_result {
                Ok(()) => None,
                Err(e) => Some(e),
            })
            .collect::<Vec<_>>();

        // Print subsystem exit states
        if failed_subsystems.is_empty() {
            log::debug!("Partial shutdown successful. Subsystem states:");
        } else {
            log::debug!("Some subsystems during partial shutdown failed. Subsystem states:");
        };
        for formatted_exit_state in formatted_exit_states {
            log::debug!("    {}", formatted_exit_state);
        }

        if failed_subsystems.is_empty() {
            Ok(())
        } else {
            Err(PartialShutdownError::SubsystemsFailed(failed_subsystems))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::shutdown_token::create_shutdown_token;

    use super::*;

    #[tokio::test]
    async fn prepare_shutdown_does_not_crash_when_called_twice() {
        let shutdown_token = create_shutdown_token();
        let data = SubsystemData::new(
            "MySubsys",
            shutdown_token.clone(),
            shutdown_token.clone(),
            CancellationToken::new(),
        );

        data.prepare_shutdown().await;
        data.prepare_shutdown().await;

        assert!(data.subsystems.lock().unwrap().is_none());
    }
}

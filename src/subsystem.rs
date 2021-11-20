use std::sync::Arc;

use anyhow::Result;
use async_recursion::async_recursion;
use futures::future::join;
use futures::future::join_all;
use std::future::Future;
use std::sync::Mutex;
use tokio::task::JoinHandle;

use crate::exit_state::join_shutdown_results;
use crate::exit_state::ShutdownResults;
use crate::exit_state::SubprocessExitState;
use crate::runner::run_subsystem;
use crate::shutdown_token::ShutdownToken;

#[cfg(doc)]
use crate::Toplevel;

pub struct SubsystemData {
    name: String,
    subsystems: Mutex<Option<Vec<SubsystemDescriptor>>>,
    shutdown_token: ShutdownToken,
}

/// The handle through which every subsystem can interact with this crate.
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
            subsystems: Mutex::new(Some(Vec::new())),
            shutdown_token,
        }
    }

    pub fn add_subsystem(
        &self,
        subsystem: Arc<SubsystemData>,
        joinhandle: JoinHandle<Result<(), ()>>,
    ) {
        match self.subsystems.lock().unwrap().as_mut() {
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
        let subsystems_taken = { self.subsystems.lock().unwrap().take() };
        let subsystems = match subsystems_taken {
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
}

impl SubsystemHandle {
    #[doc(hidden)]
    pub fn new(data: Arc<SubsystemData>) -> Self {
        Self {
            shutdown_token: data.shutdown_token.clone(),
            data,
        }
    }

    /// Starts a nested subsystem, analogous to [`Toplevel::start`].
    ///
    /// Once called, the subsystem will be started immediately, similar to [`tokio::spawn`].
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the subsystem
    /// * `subsystem` - The subsystem to be started
    ///
    /// # Examples
    ///
    /// ```
    /// use anyhow::Result;
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn nested_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    ///
    /// async fn my_subsystem(mut subsys: SubsystemHandle) -> Result<()> {
    ///     // start a nested subsystem
    ///     subsys.start("Nested", nested_subsystem);
    ///
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    /// ```
    ///
    pub fn start<
        Fut: Future<Output = Result<()>> + Send,
        S: 'static + FnOnce(SubsystemHandle) -> Fut + Send,
    >(
        &mut self,
        name: &'static str,
        subsystem: S,
    ) -> &mut Self {
        let name = {
            if !self.data.name.is_empty() {
                self.data.name.clone() + "/" + name
            } else {
                name.to_string()
            }
        };

        // Create subsystem data structure
        let new_subsystem = Arc::new(SubsystemData::new(&name, self.shutdown_token.clone()));

        // Create handle
        let subsystem_handle = SubsystemHandle::new(new_subsystem.clone());

        // Spawn new task
        let join_handle =
            tokio::spawn(async move { run_subsystem(name, subsystem, subsystem_handle).await });

        // Store subsystem data
        self.data.add_subsystem(new_subsystem, join_handle);

        self
    }

    /// Wait for the shutdown mode to be triggered.
    ///
    /// Once the shutdown mode is entered, all existing calls to this
    /// method will be released and future calls to this method will
    /// return immediately.
    ///
    /// This is the primary method of subsystems to react to
    /// the shutdown requests. Most often, it will be used in `tokio::select`
    /// statements to cancel other code as soon as the shutdown is requested.
    ///
    /// # Examples
    ///
    /// ```
    /// use anyhow::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn countdown() {
    ///     for i in (1..10).rev() {
    ///         log::info!("Countdown: {}", i);
    ///         sleep(Duration::from_millis(1000)).await;
    ///     }
    /// }
    ///
    /// async fn countdown_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     log::info!("Starting countdown ...");
    ///
    ///     // This cancels the countdown as soon as shutdown
    ///     // mode was entered
    ///     tokio::select! {
    ///         _ = subsys.on_shutdown_requested() => {
    ///             log::info!("Countdown cancelled.");
    ///         },
    ///         _ = countdown() => {
    ///             log::info!("Countdown finished.");
    ///         }
    ///     };
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn on_shutdown_requested(&self) {
        self.shutdown_token.wait_for_shutdown().await
    }

    /// Triggers the shutdown mode of the program.
    ///
    /// If a submodule itself shall have the capability to initiate a program shutdown,
    /// this is the method to use.
    ///
    /// # Examples
    ///
    /// ```
    /// use anyhow::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn stop_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem wait for one second and then stops the program.
    ///     sleep(Duration::from_millis(1000));
    ///
    ///     // An explicit shutdown request is necessary, because
    ///     // simply leaving the run() method does NOT initiate a system
    ///     // shutdown if the return value is Ok(()).
    ///     subsys.request_shutdown();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn request_shutdown(&self) {
        self.shutdown_token.shutdown()
    }

    /// Provides access to the shutdown token.
    ///
    /// This function is usually not required and is there
    /// to provide lower-level access for specific corner cases.
    #[doc(hidden)]
    pub fn shutdown_token(&self) -> &ShutdownToken {
        &self.shutdown_token
    }
}

use std::future::Future;
use std::sync::Arc;

use anyhow::Result;

use super::SubsystemData;
use super::SubsystemHandle;
use crate::runner::SubsystemRunner;
use crate::ShutdownToken;

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
        Fut: 'static + Future<Output = Result<()>> + Send,
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
        let subsystem_runner = SubsystemRunner::new(
            name,
            subsystem_handle.shutdown_token().clone(),
            subsystem(subsystem_handle),
        );

        // Store subsystem data
        self.data.add_subsystem(new_subsystem, subsystem_runner);

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

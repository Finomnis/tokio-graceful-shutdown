use std::future::Future;
use std::sync::Arc;

use super::NestedSubsystem;
use super::SubsystemData;
use super::SubsystemHandle;
use crate::runner::SubsystemRunner;
use crate::BoxedError;
use crate::PartialShutdownError;
use crate::ShutdownToken;

#[cfg(doc)]
use crate::Toplevel;

impl SubsystemHandle {
    #[doc(hidden)]
    pub fn new(data: Arc<SubsystemData>) -> Self {
        Self { data }
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
    /// # Returns
    ///
    /// A [`NestedSubsystem`] that can be used to perform a partial shutdown
    /// on the created submodule.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn nested_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    ///
    /// async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // start a nested subsystem
    ///     subsys.start("Nested", nested_subsystem);
    ///
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    /// ```
    ///
    pub fn start<
        Err: Into<BoxedError>,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
        S: 'static + FnOnce(SubsystemHandle) -> Fut + Send,
    >(
        &self,
        name: &'static str,
        subsystem: S,
    ) -> NestedSubsystem {
        let name = {
            if !self.data.name.is_empty() {
                self.data.name.clone() + "/" + name
            } else {
                name.to_string()
            }
        };

        // Create subsystem data structure
        let new_subsystem = Arc::new(SubsystemData::new(
            &name,
            self.global_shutdown_token().clone(),
            self.local_shutdown_token().child_token(),
            self.data.cancellation_token.child_token(),
        ));

        // Create handle
        let subsystem_handle = SubsystemHandle::new(new_subsystem.clone());

        // Shutdown token
        let shutdown_token = subsystem_handle.global_shutdown_token().clone();

        // Future
        let subsystem_future = async { subsystem(subsystem_handle).await.map_err(|e| e.into()) };

        // Spawn new task
        let subsystem_runner = SubsystemRunner::new(
            name,
            shutdown_token,
            new_subsystem.local_shutdown_token.clone(),
            new_subsystem.cancellation_token.child_token(),
            subsystem_future,
        );

        // Store subsystem data
        let id = self.data.add_subsystem(new_subsystem, subsystem_runner);

        NestedSubsystem { id }
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
    /// use miette::Result;
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
        self.data.local_shutdown_token.wait_for_shutdown().await
    }

    /// Triggers the shutdown mode of the program.
    ///
    /// If a submodule itself shall have the capability to initiate a program shutdown,
    /// this is the method to use.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn stop_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem wait for one second and then stops the program.
    ///     sleep(Duration::from_millis(1000)).await;
    ///
    ///     // An explicit shutdown request is necessary, because
    ///     // simply leaving the run() method does NOT initiate a program
    ///     // shutdown if the return value is Ok(()).
    ///     subsys.request_shutdown();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn request_shutdown(&self) {
        self.data.global_shutdown_token.shutdown()
    }

    /// Preforms a partial shutdown of the given nested subsystem.
    ///
    /// # Arguments
    ///
    /// * `subsystem` - The nested subsystem that should be shut down
    ///
    /// # Returns
    ///
    /// A [`PartialShutdownError`] on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use miette::Result;
    /// use tokio::time::{sleep, Duration};
    /// use tokio_graceful_shutdown::SubsystemHandle;
    ///
    /// async fn nested_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem does nothing but wait for the shutdown to happen
    ///     subsys.on_shutdown_requested().await;
    ///     Ok(())
    /// }
    ///
    /// async fn subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     // This subsystem waits for one second and then performs a partial shutdown
    ///
    ///     // Spawn nested subsystem
    ///     let nested = subsys.start("nested", nested_subsystem);
    ///
    ///     // Wait for a second
    ///     sleep(Duration::from_millis(1000)).await;
    ///
    ///     // Perform a partial shutdown of the nested subsystem
    ///     subsys.perform_partial_shutdown(nested).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn perform_partial_shutdown(
        &self,
        subsystem: NestedSubsystem,
    ) -> Result<(), PartialShutdownError> {
        self.data.perform_partial_shutdown(subsystem).await
    }

    /// Provides access to the process-wide parent shutdown token.
    ///
    /// This function is usually not required and is there
    /// to provide lower-level access for specific corner cases.
    #[doc(hidden)]
    pub fn global_shutdown_token(&self) -> &ShutdownToken {
        &self.data.global_shutdown_token
    }

    /// Provides access to the subsystem local shutdown token.
    ///
    /// This function is usually not required and is there
    /// to provide lower-level access for specific corner cases.
    #[doc(hidden)]
    pub fn local_shutdown_token(&self) -> &ShutdownToken {
        &self.data.local_shutdown_token
    }
}

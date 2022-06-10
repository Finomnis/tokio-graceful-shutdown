use crate::{errors::CancelledByShutdown, SubsystemHandle};

use pin_project_lite::pin_project;

use tokio_util::sync::WaitForCancellationFuture;

pin_project! {
    /// A Future that is resolved once the corresponding task is finished
    /// or a shutdown is initiated.
    #[must_use = "futures do nothing unless polled"]
    pub struct CancelOnShutdownFuture<'a, T: std::future::Future>{
        #[pin]
        future: T,
        #[pin]
        cancellation: WaitForCancellationFuture<'a>,
    }
}

impl<T: std::future::Future> std::future::Future for CancelOnShutdownFuture<'_, T> {
    type Output = Result<T::Output, CancelledByShutdown>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use std::task::Poll;

        let mut this = self.project();

        // Abort if there is a shutdown
        match this.cancellation.as_mut().poll(cx) {
            Poll::Ready(()) => return Poll::Ready(Err(CancelledByShutdown)),
            Poll::Pending => (),
        }

        // If there is no shutdown, see if the task is finished
        match this.future.as_mut().poll(cx) {
            Poll::Ready(res) => Poll::Ready(Ok(res)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Extends the [std::future::Future] trait with useful utility functions.
pub trait FutureExt {
    /// The type of the future.
    type Future: std::future::Future;

    /// Cancels the future when a shutdown is initiated.
    ///
    /// ## Returns
    ///
    /// A future that resolves to either the return value of the original future, or to
    /// [CancelledByShutdown] when a shutdown happened.
    ///
    /// # Arguments
    ///
    /// * `subsys` - The [SubsystemHandle] to receive the shutdown request from.
    ///
    /// # Examples
    /// ```
    /// use miette::Result;
    /// use tokio_graceful_shutdown::{errors::CancelledByShutdown, FutureExt, SubsystemHandle};
    /// use tokio::time::{sleep, Duration};
    ///
    /// async fn my_subsystem(subsys: SubsystemHandle) -> Result<()> {
    ///     match sleep(Duration::from_secs(9001))
    ///         .cancel_on_shutdown(&subsys)
    ///         .await
    ///     {
    ///         Ok(()) => {
    ///             println!("Sleep finished.");
    ///         }
    ///         Err(CancelledByShutdown) => {
    ///             println!("Sleep got cancelled by shutdown.");
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    fn cancel_on_shutdown(
        self,
        subsys: &SubsystemHandle,
    ) -> CancelOnShutdownFuture<'_, Self::Future>;
}

impl<T: std::future::Future> FutureExt for T {
    type Future = T;

    fn cancel_on_shutdown(self, subsys: &SubsystemHandle) -> CancelOnShutdownFuture<'_, T> {
        let cancellation = subsys.local_shutdown_token().wait_for_shutdown();

        CancelOnShutdownFuture {
            future: self,
            cancellation,
        }
    }
}

use crate::{errors::CancelOnShutdownError, SubsystemHandle};

use pin_project_lite::pin_project;

use tokio_util::sync::WaitForCancellationFuture;

pin_project! {
    /// A Future that is resolved once the corresponding task is finished
    /// or a shutdown is initiated.
    #[must_use = "futures do nothing unless polled"]
    pub struct CancelOnShutdownFuture<'a, T>{
        #[pin]
        future: T,
        #[pin]
        cancellation: WaitForCancellationFuture<'a>,
    }
}

impl<T> std::future::Future for CancelOnShutdownFuture<'_, T>
where
    T: std::future::Future,
{
    type Output = Result<T::Output, CancelOnShutdownError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use std::task::Poll;

        let mut this = self.project();

        // Abort if there is a shutdown
        match this.cancellation.as_mut().poll(cx) {
            Poll::Ready(()) => return Poll::Ready(Err(CancelOnShutdownError::CancelledByShutdown)),
            Poll::Pending => (),
        }

        // If there is no shutdown, see if the task is finished
        match this.future.as_mut().poll(cx) {
            Poll::Ready(res) => Poll::Ready(Ok(res)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Extends the [std::future::Future] trait with a couple of useful utility functions
pub trait FutureExt {
    /// The type of the future
    type Future;

    /// Cancels the future when a shutdown is initiated.
    ///
    /// # Arguments
    ///
    /// * `subsys` - The [SubsystemHandle] to recieve the shutdown request from.
    ///
    /// # Returns
    ///
    /// A future that resolves to the return value of the original future on success, or a
    /// [CancelOnShutdownError] when a cancellation happened.
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

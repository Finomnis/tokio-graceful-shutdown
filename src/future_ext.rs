use std::marker::PhantomData;

use crate::{errors::CancelOnShutdownError, SubsystemHandle};

use pin_project_lite::pin_project;

use tokio_util::sync::WaitForCancellationFuture;

pin_project! {
    /// aaa
    #[must_use = "futures do nothing unless polled"]
    pub struct CancelOnShutdownFuture<'a, 'b, T: 'a>{
        //future: Pin<Box<dyn std::future::Future<Output = Result<T, CancelOnShutdownError>> + Send + Sync >>,
        #[pin]
        future: T,
        #[pin]
        cancellation: WaitForCancellationFuture<'b>,
        _p: PhantomData<&'a T>
    }
}

// impl<T> std::future::Future for CancelOnShutdownFuture<T> {
//     type Output = Result<T, CancelOnShutdownError>;

//     fn poll(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Self::Output> {
//         let this = self.project();
//         this.f.poll(cx)
//     }
// }

impl<'a, 'b, T> std::future::Future for CancelOnShutdownFuture<'a, 'b, T>
where
    T: std::future::Future + Send + Sync + 'a,
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
pub trait FutureExt<'a, 'b> {
    /// The return type of the future
    type Output;

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
        subsys: &'b SubsystemHandle,
    ) -> CancelOnShutdownFuture<'a, 'b, Self::Output>;
}

impl<'a, 'b, T> FutureExt<'a, 'b> for T
where
    T: std::future::Future + Send + Sync + 'a,
{
    type Output = T;

    fn cancel_on_shutdown(
        self,
        subsys: &'b SubsystemHandle,
    ) -> CancelOnShutdownFuture<'a, 'b, Self::Output> {
        let cancellation = subsys.local_shutdown_token().wait_for_shutdown();

        CancelOnShutdownFuture {
            future: self,
            cancellation,
            _p: PhantomData,
        }
    }
}

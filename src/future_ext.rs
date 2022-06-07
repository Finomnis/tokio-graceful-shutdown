use std::{marker::PhantomData, pin::Pin};

use crate::{errors::CancelOnShutdownError, SubsystemHandle};

use pin_project_lite::pin_project;

pin_project! {
    /// aaa
    #[must_use = "futures do nothing unless polled"]
    pub struct CancelOnShutdownFuture<'a, T: 'a>{
        //future: Pin<Box<dyn std::future::Future<Output = Result<T, CancelOnShutdownError>> + Send + Sync >>,
        #[pin]
        f: T,
        x: PhantomData<&'a T>
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

impl<'a, T> std::future::Future for CancelOnShutdownFuture<'a, T>
where
    T: std::future::Future + Send + Sync + 'a,
{
    type Output = T::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.project();
        this.f.as_mut().poll(cx)
    }
}

/// Extends the [std::future::Future] trait with a couple of useful utility functions
pub trait FutureExt<'a> {
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
        subsys: &SubsystemHandle,
    ) -> CancelOnShutdownFuture<'a, Self::Output>;
}

fn create_cancel_on_shutdown_future<'a, T>(
    f: T,
    subsys: SubsystemHandle,
) -> CancelOnShutdownFuture<'a, T>
where
    T: std::future::Future + Send + Sync + 'a,
{
    // let future = Box::pin(async move {
    //     let x = f;
    //     tokio::select! {
    //         _ = subsys.on_shutdown_requested() => Err(CancelOnShutdownError::CancelledByShutdown),
    //         res = f => Ok(res)
    //     }
    // });

    CancelOnShutdownFuture { f, x: PhantomData }
}

impl<'a, T> FutureExt<'a> for T
where
    T: std::future::Future + Send + Sync + 'a,
{
    type Output = T;

    fn cancel_on_shutdown(
        self,
        subsys: &SubsystemHandle,
    ) -> CancelOnShutdownFuture<'a, Self::Output> {
        let subsys = subsys.clone();

        create_cancel_on_shutdown_future(self, subsys)
        //todo!()
    }
}

//! The SubsystemRunner is a little tricky, so here some explanation.
//!
//! A two-layer `tokio::spawn` is required to make this work reliably; the inner `spawn` is the actual subsystem,
//! and the outer `spawn` carries out the duty of propagating the `StopReason` and cleaning up.
//!
//! Further, everything in here reacts properly to being dropped, including
//! the runner itself, who cancels the subsystem on drop.

use std::{future::Future, sync::Arc};

use crate::{
    errors::{SubsystemError, SubsystemFailure},
    ErrTypeTraits, SubsystemHandle,
};

mod alive_guard;
pub(crate) use self::alive_guard::AliveGuard;

pub(crate) struct SubsystemRunner {
    aborthandle: tokio::task::AbortHandle,
}

impl SubsystemRunner {
    #[track_caller]
    pub(crate) fn new<Fut, Subsys, ErrType: ErrTypeTraits, Err>(
        name: Arc<str>,
        subsystem: Subsys,
        subsystem_handle: SubsystemHandle<ErrType>,
        guard: AliveGuard,
    ) -> Self
    where
        Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
        Err: Into<ErrType>,
    {
        let future = run_subsystem(name, subsystem, subsystem_handle, guard);
        let aborthandle = crate::tokio_task::spawn(future, "subsystem_runner").abort_handle();
        SubsystemRunner { aborthandle }
    }

    pub(crate) fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.aborthandle.clone()
    }
}

impl Drop for SubsystemRunner {
    fn drop(&mut self) {
        self.aborthandle.abort()
    }
}

#[track_caller]
fn run_subsystem<Fut, Subsys, ErrType: ErrTypeTraits, Err>(
    name: Arc<str>,
    subsystem: Subsys,
    mut subsystem_handle: SubsystemHandle<ErrType>,
    guard: AliveGuard,
) -> impl Future<Output = ()> + 'static
where
    Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
    Fut: 'static + Future<Output = Result<(), Err>> + Send,
    Err: Into<ErrType>,
{
    let mut redirected_subsystem_handle = subsystem_handle.delayed_clone();

    let future = async { subsystem(subsystem_handle).await.map_err(|e| e.into()) };
    let join_handle = crate::tokio_task::spawn(future, &name);

    // Abort on drop
    guard.on_cancel({
        let abort_handle = join_handle.abort_handle();
        let name = Arc::clone(&name);
        move || {
            if !abort_handle.is_finished() {
                tracing::warn!("Subsystem cancelled: '{}'", name);
            }
            abort_handle.abort();
        }
    });

    async move {
        // Move guard into here, to tie it to the scope of the async
        let _guard = guard;

        let failure = match join_handle.await {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(SubsystemError::Failed(name, SubsystemFailure(e))),
            Err(e) => {
                // We can assume that this is a panic, because a cancellation
                // can never happen as long as we still hold `guard`.
                assert!(e.is_panic());
                Some(SubsystemError::Panicked(name))
            }
        };

        // Retrieve the handle that was passed into the subsystem.
        // Originally it was intended to pass the handle as reference, but due
        // to complications (https://stackoverflow.com/a/70592053/2902833 and
        // https://github.com/tokio-rs/tokio/issues/3162) it was decided to
        // pass ownership instead.
        //
        // It is still important that the handle does not leak out of the subsystem.
        let subsystem_handle = match redirected_subsystem_handle.try_recv() {
            Ok(s) => s,
            Err(_) => {
                tracing::error!(
                    "The SubsystemHandle object must not be leaked out of the subsystem!"
                );
                panic!("The SubsystemHandle object must not be leaked out of the subsystem!");
            }
        };

        // Raise potential errors
        let joiner_token = subsystem_handle.joiner_token;
        if let Some(failure) = failure {
            joiner_token.raise_failure(failure);
        }

        // Wait for children to finish before we destroy the `SubsystemHandle` object.
        // Otherwise the children would be cancelled immediately.
        //
        // This is the main mechanism that forwards a cancellation to all the children.
        joiner_token.downgrade().join().await;
    }
}

//! The SubsystemRunner is a little tricky, so here some explanation.
//!
//! A two-layer `tokio::spawn` is required to make this work reliably; the inner `spawn` is the actual subsystem,
//! and the outer `spawn` carries out the duty of propagating the `StopReason`.
//!
//! Further, everything in here reacts properly to being dropped, including the `AliveGuard` (propagating `StopReason::Cancel` in that case)
//! and runner itself, who cancels the subsystem on drop.

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
        let future = async { run_subsystem(name, subsystem, subsystem_handle, guard).await };
        let aborthandle = tokio::spawn(future).abort_handle();
        SubsystemRunner { aborthandle }
    }
}

impl Drop for SubsystemRunner {
    fn drop(&mut self) {
        self.aborthandle.abort()
    }
}

async fn run_subsystem<Fut, Subsys, ErrType: ErrTypeTraits, Err>(
    name: Arc<str>,
    subsystem: Subsys,
    mut subsystem_handle: SubsystemHandle<ErrType>,
    guard: AliveGuard,
) where
    Subsys: 'static + FnOnce(SubsystemHandle<ErrType>) -> Fut + Send,
    Fut: 'static + Future<Output = Result<(), Err>> + Send,
    Err: Into<ErrType>,
{
    let mut redirected_subsystem_handle = subsystem_handle.delayed_clone();

    let future = async { subsystem(subsystem_handle).await.map_err(|e| e.into()) };
    let join_handle = tokio::spawn(future);

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

    let failure = match join_handle.await {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(SubsystemError::Failed(name, SubsystemFailure(e))),
        Err(e) => {
            if e.is_panic() {
                Some(SubsystemError::Panicked(name))
            } else {
                // Don't do anything in case of a cancellation;
                // cancellations can't be forwarded (because the
                // current function we are in will be cancelled
                // simultaneously)
                None
            }
        }
    };

    // Retrieve the handle that was passed into the subsystem.
    // Originally it was intended to pass the handle as reference, but due
    // to complications (https://stackoverflow.com/questions/77172947/async-lifetime-issues-of-pass-by-reference-parameters)
    // it was decided to pass ownership instead.
    //
    // It is still important that the handle does not leak out of the subsystem.
    let subsystem_handle = match redirected_subsystem_handle.try_recv() {
        Ok(s) => s,
        Err(_) => panic!("The SubsystemHandle object must not be leaked out of the subsystem!"),
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

/*
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::{
        sync::oneshot,
        time::{timeout, Duration},
    };

    use super::*;
    use crate::{subsystem::root_handle, BoxedError};

    fn create_result_and_guard() -> (oneshot::Receiver<StopReason>, AliveGuard) {
        let (sender, receiver) = oneshot::channel();

        let guard = AliveGuard::new();
        guard.on_finished({
            move |r| {
                sender.send(r).unwrap();
            }
        });

        (receiver, guard)
    }

    mod run_subsystem {

        use super::*;

        #[tokio::test]
        async fn finish() {
            let (mut result, guard) = create_result_and_guard();

            run_subsystem(
                Arc::from(""),
                |_| async { Result::<(), BoxedError>::Ok(()) },
                root_handle(),
                guard,
            )
            .await;

            assert!(matches!(result.try_recv(), Ok(StopReason::Finish)));
        }

        #[tokio::test]
        async fn panic() {
            let (mut result, guard) = create_result_and_guard();

            run_subsystem::<_, _, _, BoxedError>(
                Arc::from(""),
                |_| async {
                    panic!();
                },
                root_handle(),
                guard,
            )
            .await;

            assert!(matches!(result.try_recv(), Ok(StopReason::Panic)));
        }

        #[tokio::test]
        async fn error() {
            let (mut result, guard) = create_result_and_guard();

            run_subsystem::<_, _, _, BoxedError>(
                Arc::from(""),
                |_| async { Err(String::from("").into()) },
                root_handle(),
                guard,
            )
            .await;

            assert!(matches!(result.try_recv(), Ok(StopReason::Error(_))));
        }

        #[tokio::test]
        async fn cancelled_with_delay() {
            let (mut result, guard) = create_result_and_guard();

            let (drop_sender, mut drop_receiver) = tokio::sync::mpsc::channel::<()>(1);

            let timeout_result = timeout(
                Duration::from_millis(100),
                run_subsystem::<_, _, _, BoxedError>(
                    Arc::from(""),
                    |_| async move {
                        drop_sender.send(()).await.unwrap();
                        std::future::pending().await
                    },
                    root_handle(),
                    guard,
                ),
            )
            .await;

            assert!(timeout_result.is_err());
            drop(timeout_result);

            // Make sure we are executing the subsystem
            let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
                .await
                .unwrap();
            assert!(recv_result.is_some());

            // Make sure the subsystem got cancelled
            let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
                .await
                .unwrap();
            assert!(recv_result.is_none());

            assert!(matches!(result.try_recv(), Ok(StopReason::Cancelled)));
        }

        #[tokio::test]
        async fn cancelled_immediately() {
            let (mut result, guard) = create_result_and_guard();

            let (drop_sender, mut drop_receiver) = tokio::sync::mpsc::channel::<()>(1);

            let _ = run_subsystem::<_, _, _, BoxedError>(
                Arc::from(""),
                |_| async move {
                    drop_sender.send(()).await.unwrap();
                    std::future::pending().await
                },
                root_handle(),
                guard,
            );

            // Make sure we are executing the subsystem
            let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
                .await
                .unwrap();
            assert!(recv_result.is_none());

            assert!(matches!(result.try_recv(), Ok(StopReason::Cancelled)));
        }
    }

    mod subsystem_runner {
        use crate::utils::JoinerToken;

        use super::*;

        #[tokio::test]
        async fn finish() {
            let (mut result, guard) = create_result_and_guard();

            let runner = SubsystemRunner::new(
                Arc::from(""),
                |_| async { Result::<(), BoxedError>::Ok(()) },
                root_handle(),
                guard,
            );

            let result = timeout(Duration::from_millis(200), result).await.unwrap();
            assert!(matches!(result, Ok(StopReason::Finish)));
        }

        #[tokio::test]
        async fn panic() {
            let (mut result, guard) = create_result_and_guard();

            let runner = SubsystemRunner::new::<_, _, _, BoxedError>(
                Arc::from(""),
                |_| async {
                    panic!();
                },
                root_handle(),
                guard,
            );

            let result = timeout(Duration::from_millis(200), result).await.unwrap();
            assert!(matches!(result, Ok(StopReason::Panic)));
        }

        #[tokio::test]
        async fn error() {
            let (mut result, guard) = create_result_and_guard();

            let runner = SubsystemRunner::new::<_, _, _, BoxedError>(
                Arc::from(""),
                |_| async { Err(String::from("").into()) },
                root_handle(),
                guard,
            );

            let result = timeout(Duration::from_millis(200), result).await.unwrap();
            assert!(matches!(result, Ok(StopReason::Error(_))));
        }

        #[tokio::test]
        async fn cancelled_with_delay() {
            let (mut result, guard) = create_result_and_guard();

            let (drop_sender, mut drop_receiver) = tokio::sync::mpsc::channel::<()>(1);

            let runner = SubsystemRunner::new::<_, _, _, BoxedError>(
                Arc::from(""),
                |_| async move {
                    drop_sender.send(()).await.unwrap();
                    std::future::pending().await
                },
                root_handle(),
                guard,
            );

            // Make sure we are executing the subsystem
            let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
                .await
                .unwrap();
            assert!(recv_result.is_some());

            drop(runner);

            // Make sure the subsystem got cancelled
            let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
                .await
                .unwrap();
            assert!(recv_result.is_none());

            let result = timeout(Duration::from_millis(200), result).await.unwrap();
            assert!(matches!(result, Ok(StopReason::Cancelled)));
        }

        #[tokio::test]
        async fn cancelled_immediately() {
            let (mut result, guard) = create_result_and_guard();

            let (mut joiner_token, _) = JoinerToken::new(|_| None);

            let _ = SubsystemRunner::new::<_, _, _, BoxedError>(
                Arc::from(""),
                {
                    let (joiner_token, _) = joiner_token.child_token(|_| None);
                    |_| async move {
                        let joiner_token = joiner_token;
                        std::future::pending().await
                    }
                },
                root_handle(),
                guard,
            );

            // Make sure the subsystem got cancelled
            timeout(Duration::from_millis(100), joiner_token.join_children())
                .await
                .unwrap();

            let result = timeout(Duration::from_millis(200), result).await.unwrap();
            assert!(matches!(result, Ok(StopReason::Cancelled)));
        }
    }
}
*/

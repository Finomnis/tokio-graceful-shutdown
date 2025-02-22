pub mod common;
use std::sync::Arc;

use futures::future::BoxFuture;
use std::sync::atomic::{self, AtomicBool};

use common::BoxedResult;
use futures::FutureExt;
use std::convert::Infallible;
use tokio::time::Duration;
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing_test::traced_test;

#[tokio::test(start_paused = true)]
#[traced_test]
async fn abort_subsystem_works() {
    // Diagram:
    //
    // top
    //   \
    //    nested (rcv's abort at 0.5s, panics after 1s)

    let subsys_nested = move |_: SubsystemHandle| -> BoxFuture<BoxedResult> {
        async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            panic!("Nested subsystem should not reach completion");
        }
        .boxed()
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("subsys_nested", subsys_nested));

        tokio::time::sleep(Duration::from_millis(500)).await;
        nested.abort();
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(nested.is_finished());

        tokio::time::sleep(Duration::from_millis(1000)).await;

        Ok::<_, Infallible>(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys_top", subsys_top));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn nested_subsystem_is_aborted() {
    // Diagram:
    //
    // top
    //   \
    //    d1 (0s lifetime, rcv's abort at 0.5s)
    //     \
    //      d2 (panics after 1s)
    //
    // We want to ensure aborting d1 aborts d2.

    let subsys_nested_d2 = move |_: SubsystemHandle| -> BoxFuture<BoxedResult> {
        async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            panic!("Depth 2 subsystem should not reach completion");
        }
        .boxed()
    };

    let subsys_nested_d1 = async move |subsys: SubsystemHandle| {
        let _nested = subsys.start(SubsystemBuilder::new("d2", subsys_nested_d2));
        BoxedResult::Ok(())
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("d1", subsys_nested_d1));

        tokio::time::sleep(Duration::from_millis(500)).await;
        nested.abort();
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(nested.is_finished());

        tokio::time::sleep(Duration::from_millis(1000)).await;

        BoxedResult::Ok(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys_top", subsys_top));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn multiple_abort_works() {
    // Diagram:
    //
    // top
    //   \
    //    nested (rcv's abort at 0.5s, rcv's abort at 0.6s, panics at 1s)
    //
    // This is just making sure we can call .abort() multiple times without
    // problems happening.

    let subsys_nested = move |_: SubsystemHandle| -> BoxFuture<BoxedResult> {
        async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            panic!("Nested subsystem should not reach completion");
        }
        .boxed()
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("subsys_nested", subsys_nested));

        tokio::time::sleep(Duration::from_millis(500)).await;
        nested.abort();

        tokio::time::sleep(Duration::from_millis(100)).await;
        nested.abort();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        Ok::<_, Infallible>(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys_top", subsys_top));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn abort_overrides_shutdown() {
    // Diagram:
    //
    // top
    //   \
    //    nested (rcv's shutdown at 0.5s, rcv's abort at 0.6s, shuts down at 1s after shutdown requested)

    let subsys_nested = move |s: SubsystemHandle| -> BoxFuture<BoxedResult> {
        async move {
            s.on_shutdown_requested().await;
            tracing::info!("received shutdown signal");
            tokio::time::sleep(Duration::from_millis(500)).await;

            panic!("Nested subsystem should not reach completion");
        }
        .boxed()
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("subsys_nested", subsys_nested));

        tokio::time::sleep(Duration::from_millis(500)).await;
        nested.initiate_shutdown();

        tokio::time::sleep(Duration::from_millis(100)).await;
        nested.abort();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        Ok::<_, Infallible>(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys_top", subsys_top));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn abort_ensures_drop() {
    // Diagram:
    //
    // top
    //   \
    //    nested (rcv's abort at 0.5s, owns an object that we expect to be dropped, 1s lifetime)

    /// Holds reference to a flag. The flag is initialized to false.
    /// When this object is dropped, the flag is set to true.
    struct IHaveNoMouthYetIMustBeDropped {
        was_dropped: Arc<AtomicBool>,
    }
    impl IHaveNoMouthYetIMustBeDropped {
        fn new() -> Self {
            Self {
                was_dropped: Arc::new(AtomicBool::new(false)),
            }
        }
    }
    impl Drop for IHaveNoMouthYetIMustBeDropped {
        fn drop(&mut self) {
            self.was_dropped.store(true, atomic::Ordering::Relaxed);
        }
    }

    let subsys_top = async move |subsys: SubsystemHandle| {
        let to_be_dropped = IHaveNoMouthYetIMustBeDropped::new();
        let flag = to_be_dropped.was_dropped.clone();

        let nested = subsys.start(SubsystemBuilder::new(
            "subsys_nested",
            move |_s: SubsystemHandle| -> BoxFuture<BoxedResult> {
                async move {
                    let _owned_object = to_be_dropped; //take ownership of the drop object
                    loop {
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                }
                .boxed()
            },
        ));

        tokio::time::sleep(Duration::from_millis(500)).await;
        nested.abort();
        tokio::time::sleep(Duration::from_millis(1)).await; // may need to wait for it to be dropped

        assert!(
            flag.load(atomic::Ordering::Relaxed),
            "drop did not get called"
        );

        Ok::<_, Infallible>(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys_top", subsys_top));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

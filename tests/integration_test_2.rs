use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing_test::traced_test;

pub mod common;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::common::Event;
use common::BoxedResult;

#[tokio::test(start_paused = true)]
#[traced_test]
async fn leak_subsystem_handle() {
    let subsys_ext: Arc<Mutex<Option<SubsystemHandle>>> = Default::default();
    let subsys_ext2 = Arc::clone(&subsys_ext);

    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;

        *subsys_ext2.lock().unwrap() = Some(subsys);

        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(100))
        .await;
    assert!(result.is_err());
    assert!(logs_contain(
        "The SubsystemHandle object must not be leaked out of the subsystem!"
    ));
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn wait_for_children() {
    let (nested1_started, set_nested1_started) = Event::create();
    let (nested1_finished, set_nested1_finished) = Event::create();
    let (nested2_started, set_nested2_started) = Event::create();
    let (nested2_finished, set_nested2_finished) = Event::create();

    let nested_subsys2 = async move |subsys: SubsystemHandle| {
        set_nested2_started();
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        set_nested2_finished();
        BoxedResult::Ok(())
    };

    let nested_subsys1 = async move |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested2", nested_subsys2));
        set_nested1_started();
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        set_nested1_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = async move |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested1", nested_subsys1));

        sleep(Duration::from_millis(100)).await;

        subsys.request_shutdown();

        assert!(nested1_started.get());
        assert!(!nested1_finished.get());
        assert!(nested2_started.get());
        assert!(!nested2_finished.get());

        subsys.wait_for_children().await;

        assert!(nested1_finished.get());
        assert!(nested2_finished.get());

        BoxedResult::Ok(())
    };

    Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn request_local_shutdown() {
    let (nested1_started, set_nested1_started) = Event::create();
    let (nested1_finished, set_nested1_finished) = Event::create();
    let (nested2_started, set_nested2_started) = Event::create();
    let (nested2_finished, set_nested2_finished) = Event::create();
    let (global_finished, set_global_finished) = Event::create();

    let nested_subsys2 = async move |subsys: SubsystemHandle| {
        set_nested2_started();
        subsys.on_shutdown_requested().await;
        set_nested2_finished();
        BoxedResult::Ok(())
    };

    let nested_subsys1 = async move |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested2", nested_subsys2));
        set_nested1_started();
        subsys.on_shutdown_requested().await;
        set_nested1_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = async move |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested1", nested_subsys1));

        sleep(Duration::from_millis(100)).await;

        assert!(nested1_started.get());
        assert!(!nested1_finished.get());
        assert!(nested2_started.get());
        assert!(!nested2_finished.get());
        assert!(!global_finished.get());
        assert!(!subsys.is_shutdown_requested());

        subsys.request_local_shutdown();
        sleep(Duration::from_millis(200)).await;

        assert!(nested1_finished.get());
        assert!(nested2_finished.get());
        assert!(!global_finished.get());
        assert!(subsys.is_shutdown_requested());

        subsys.request_shutdown();
        sleep(Duration::from_millis(50)).await;

        assert!(global_finished.get());
        assert!(subsys.is_shutdown_requested());

        BoxedResult::Ok(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsys1));

        s.on_shutdown_requested().await;
        set_global_finished();
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

#[cfg(unix)]
#[tokio::test]
#[traced_test]
async fn shutdown_through_signal_2() {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;
    use tokio_graceful_shutdown::FutureExt;

    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Ok(())
    };

    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;

            // Send SIGINT to ourselves.
            signal::kill(Pid::this(), Signal::SIGTERM).unwrap();
        },
        async {
            let result = Toplevel::new(async move |s| {
                s.start(SubsystemBuilder::new("subsys", subsystem));
                assert!(
                    sleep(Duration::from_millis(1000))
                        .cancel_on_shutdown(&s)
                        .await
                        .is_err()
                );
                assert!(s.is_shutdown_requested());
            })
            .catch_signals()
            .handle_shutdown_requests(Duration::from_millis(400))
            .await;
            assert!(result.is_ok());
        },
    );
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn cancellation_token() {
    let subsystem = async |subsys: SubsystemHandle| {
        let cancellation_token = subsys.create_cancellation_token();

        assert!(!cancellation_token.is_cancelled());
        subsys.on_shutdown_requested().await;
        assert!(cancellation_token.is_cancelled());

        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(400))
        .await;
    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn cancellation_token_does_not_propagate_up() {
    let subsystem = async |subsys: SubsystemHandle| {
        let cancellation_token = subsys.create_cancellation_token();

        cancellation_token.cancel();
        assert!(!subsys.is_shutdown_requested());

        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(400))
        .await;
    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_finished_works_correctly() {
    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        let nested = s.start(SubsystemBuilder::new("subsys", subsystem));
        let nested_finished = nested.finished();

        let is_finished = AtomicBool::new(false);
        tokio::join!(
            async {
                nested_finished.await;
                is_finished.store(true, Ordering::Release);
            },
            async {
                sleep(Duration::from_millis(20)).await;
                assert!(!is_finished.load(Ordering::Acquire));
                nested.initiate_shutdown();
                sleep(Duration::from_millis(20)).await;
                assert!(is_finished.load(Ordering::Acquire));
            }
        );
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(400))
        .await;
    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn shutdown_does_not_propagate_to_detached_subsystem() {
    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let detached_subsystem = async |subsys: SubsystemHandle| {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("detached", detached_subsystem).detached());
        sleep(Duration::from_millis(20)).await;
        assert!(nested_started.get());
        assert!(!nested_finished.get());

        subsys.on_shutdown_requested().await;

        sleep(Duration::from_millis(20)).await;
        assert!(!nested_finished.get());

        nested.initiate_shutdown();

        sleep(Duration::from_millis(20)).await;
        assert!(nested_finished.get());

        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(400))
        .await;
    assert!(result.is_ok());
}

use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing_test::traced_test;

pub mod common;

use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use crate::common::Event;

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
#[traced_test]
async fn leak_subsystem_handle() {
    let subsys_ext: Arc<Mutex<Option<SubsystemHandle>>> = Default::default();
    let subsys_ext2 = Arc::clone(&subsys_ext);

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;

        *subsys_ext2.lock().unwrap() = Some(subsys);

        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
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

#[tokio::test]
#[traced_test]
async fn wait_for_children() {
    let (nested1_started, set_nested1_started) = Event::create();
    let (nested1_finished, set_nested1_finished) = Event::create();
    let (nested2_started, set_nested2_started) = Event::create();
    let (nested2_finished, set_nested2_finished) = Event::create();

    let nested_subsys2 = move |subsys: SubsystemHandle| async move {
        set_nested2_started();
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        set_nested2_finished();
        BoxedResult::Ok(())
    };

    let nested_subsys1 = move |subsys: SubsystemHandle| async move {
        subsys.start(SubsystemBuilder::new("nested2", nested_subsys2));
        set_nested1_started();
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        set_nested1_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
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

    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await
    .unwrap();
}

#[tokio::test]
#[traced_test]
async fn request_local_shutdown() {
    let (nested1_started, set_nested1_started) = Event::create();
    let (nested1_finished, set_nested1_finished) = Event::create();
    let (nested2_started, set_nested2_started) = Event::create();
    let (nested2_finished, set_nested2_finished) = Event::create();
    let (global_finished, set_global_finished) = Event::create();

    let nested_subsys2 = move |subsys: SubsystemHandle| async move {
        set_nested2_started();
        subsys.on_shutdown_requested().await;
        set_nested2_finished();
        BoxedResult::Ok(())
    };

    let nested_subsys1 = move |subsys: SubsystemHandle| async move {
        subsys.start(SubsystemBuilder::new("nested2", nested_subsys2));
        set_nested1_started();
        subsys.on_shutdown_requested().await;
        set_nested1_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        subsys.start(SubsystemBuilder::new("nested1", nested_subsys1));

        sleep(Duration::from_millis(100)).await;

        assert!(nested1_started.get());
        assert!(!nested1_finished.get());
        assert!(nested2_started.get());
        assert!(!nested2_finished.get());
        assert!(!global_finished.get());

        subsys.request_local_shutdown();
        sleep(Duration::from_millis(200)).await;

        assert!(nested1_finished.get());
        assert!(nested2_finished.get());
        assert!(!global_finished.get());

        subsys.request_shutdown();
        sleep(Duration::from_millis(50)).await;

        assert!(global_finished.get());

        BoxedResult::Ok(())
    };

    Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys1));

        s.on_shutdown_requested().await;
        set_global_finished();
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

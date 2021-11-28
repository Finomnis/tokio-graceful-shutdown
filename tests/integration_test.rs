use anyhow::anyhow;
use tokio::time::{sleep, timeout, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

mod common;
use common::event::Event;

#[tokio::test]
async fn normal_shutdown() {
    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;
            shutdown_token.shutdown();
        },
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(400))
                .await;
            assert!(result.is_ok());
        },
    );
}

#[tokio::test]
async fn shutdown_timeout_causes_error() {
    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(400)).await;
        Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;
            shutdown_token.shutdown();
        },
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(200))
                .await;
            assert!(result.is_err());
        },
    );
}

#[tokio::test]
async fn subsystem_finishes_with_success() {
    let subsystem = |_| async { Ok(()) };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(100))
                .await;
            set_toplevel_finished();
            // Assert Ok(()) returncode properly propagates to Toplevel
            assert!(result.is_ok());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            // Assert Ok(()) doesn't cause a shutdown
            assert!(!toplevel_finished.get());
            shutdown_token.shutdown();
            sleep(Duration::from_millis(200)).await;
            // Assert toplevel sucessfully gets stopped, nothing hangs
            assert!(toplevel_finished.get());
        },
    );
}

#[tokio::test]
async fn subsystem_finishes_with_error() {
    let subsystem = |_| async { Err(anyhow!("Error!")) };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(100))
                .await;
            set_toplevel_finished();
            // Assert Err(()) returncode properly propagates to Toplevel
            assert!(result.is_err());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            // Assert Err(()) causes a shutdown
            assert!(toplevel_finished.get());
            assert!(shutdown_token.is_shutting_down());
        },
    );
}

#[tokio::test]
async fn subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsys);
    let shutdown_token = toplevel.get_shutdown_token().clone();
    let result = tokio::spawn(toplevel.handle_shutdown_requests(Duration::from_millis(100)));

    sleep(Duration::from_millis(100)).await;
    assert!(!subsys_finished.get());

    shutdown_token.shutdown();
    timeout(Duration::from_millis(100), subsys_finished.wait())
        .await
        .unwrap();

    let result = timeout(Duration::from_millis(100), result)
        .await
        .unwrap()
        .unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn nested_subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        Ok(())
    };

    let subsystem = |mut subsys: SubsystemHandle| async move {
        subsys.start("nested", nested_subsystem);
        subsys.on_shutdown_requested().await;
        Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();
    let result = tokio::spawn(toplevel.handle_shutdown_requests(Duration::from_millis(100)));

    sleep(Duration::from_millis(100)).await;
    assert!(!subsys_finished.get());

    shutdown_token.shutdown();
    timeout(Duration::from_millis(100), subsys_finished.wait())
        .await
        .unwrap();

    let result = timeout(Duration::from_millis(100), result)
        .await
        .unwrap()
        .unwrap();

    assert!(result.is_ok());
}

#[tokio::test]
async fn nested_subsystem_error_propagates() {
    let nested_subsystem = |_subsys: SubsystemHandle| async move { Err(anyhow!("Error!")) };

    let subsystem = move |mut subsys: SubsystemHandle| async move {
        subsys.start("nested", nested_subsystem);
        subsys.on_shutdown_requested().await;
        Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(100))
                .await;
            set_toplevel_finished();
            // Assert Err(()) returncode properly propagates to Toplevel
            assert!(result.is_err());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            // Assert Err(()) causes a shutdown
            assert!(toplevel_finished.get());
            assert!(shutdown_token.is_shutting_down());
        },
    );
}

#[tokio::test]
async fn panic_gets_handled_correctly() {
    let nested_subsystem = |_subsys: SubsystemHandle| async move {
        panic!("Error!");
    };

    let subsystem = move |mut subsys: SubsystemHandle| async move {
        subsys.start("nested", nested_subsystem);
        subsys.on_shutdown_requested().await;
        Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(100))
                .await;
            set_toplevel_finished();
            // Assert panic causes Error propagation to Toplevel
            assert!(result.is_err());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            // Assert panic causes a shutdown
            assert!(toplevel_finished.get());
            assert!(shutdown_token.is_shutting_down());
        },
    );
}

#[tokio::test]
async fn subsystem_can_request_shutdown() {
    let (subsystem_should_stop, stop_subsystem) = Event::create();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsystem_should_stop.wait().await;
        subsys.request_shutdown();
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(100))
                .await;
            set_toplevel_finished();

            // Assert graceful shutdown does not cause an Error code
            assert!(result.is_ok());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            assert!(!toplevel_finished.get());
            assert!(!subsys_finished.get());
            assert!(!shutdown_token.is_shutting_down());

            stop_subsystem();
            sleep(Duration::from_millis(200)).await;

            // Assert request_shutdown() causes a shutdown
            assert!(toplevel_finished.get());
            assert!(subsys_finished.get());
            assert!(shutdown_token.is_shutting_down());
        },
    );
}

#[tokio::test]
async fn shutdown_timeout_causes_cancellation() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(300)).await;
        set_subsys_finished();
        Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(200))
                .await;
            set_toplevel_finished();

            // Assert graceful shutdown does not cause an Error code
            assert!(result.is_err());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            assert!(!toplevel_finished.get());
            assert!(!subsys_finished.get());
            assert!(!shutdown_token.is_shutting_down());

            shutdown_token.shutdown();
            timeout(Duration::from_millis(300), toplevel_finished.wait())
                .await
                .unwrap();

            // Assert shutdown timed out causes a shutdown
            assert!(toplevel_finished.get());
            assert!(!subsys_finished.get());

            // Assert subsystem was canceled and didn't continue running in the background
            sleep(Duration::from_millis(500)).await;
            assert!(!subsys_finished.get());
        },
    );
}

#[tokio::test]
async fn spawning_task_during_shutdown_causes_task_to_be_cancelled() {
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested = |_: SubsystemHandle| async move {
        sleep(Duration::from_millis(100)).await;
        set_nested_finished();
        Ok(())
    };

    let subsystem = move |mut subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        subsys.start("Nested", nested);
        set_subsys_finished();
        Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(500))
                .await;
            set_toplevel_finished();

            // Assert graceful shutdown does not cause an Error code
            assert!(result.is_ok());
        },
        async {
            sleep(Duration::from_millis(200)).await;
            assert!(!toplevel_finished.get());
            assert!(!subsys_finished.get());
            assert!(!shutdown_token.is_shutting_down());
            assert!(!nested_finished.get());

            shutdown_token.shutdown();
            timeout(Duration::from_millis(200), toplevel_finished.wait())
                .await
                .unwrap();

            // Assert that subsystem did not get past spawning the task, as spawning a task while shutting
            // down causes a panic.
            assert!(subsys_finished.get());
            assert!(!nested_finished.get());

            // Assert nested was canceled and didn't continue running in the background
            sleep(Duration::from_millis(500)).await;
            assert!(!nested_finished.get());
        },
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn double_panic_does_not_stop_graceful_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys3 = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(400)).await;
        set_subsys_finished();
        Ok(())
    };

    let subsys2 = |_subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem2 panicked!")
    };

    let subsys1 = move |mut subsys: SubsystemHandle| async move {
        subsys.start("Subsys2", subsys2);
        subsys.start("Subsys3", subsys3);
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem1 panicked!")
    };

    let result = Toplevel::new()
        .start("subsys", subsys1)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;
    assert!(result.is_err());

    assert!(subsys_finished.get());
}

#[tokio::test]
async fn destroying_toplevel_cancels_subsystems() {
    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys1 = move |_subsys: SubsystemHandle| async move {
        set_subsys_started();
        sleep(Duration::from_millis(100)).await;
        set_subsys_finished();
        Ok(())
    };

    {
        let _result = Toplevel::new().start("subsys", subsys1);
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

#[tokio::test]
async fn destroying_toplevel_cancels_nested_toplevel_subsystems() {
    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys2 = move |_subsys: SubsystemHandle| async move {
        set_subsys_started();
        sleep(Duration::from_millis(100)).await;
        set_subsys_finished();
        Ok(())
    };

    let subsys1 = move |_subsys: SubsystemHandle| async move {
        Toplevel::new()
            .start("subsys2", subsys2)
            .handle_shutdown_requests(Duration::from_millis(100))
            .await
    };

    {
        let _result = Toplevel::new().start("subsys", subsys1);
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

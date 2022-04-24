use anyhow::anyhow;
use env_logger;
use tokio::time::{sleep, timeout, Duration};
use tokio_graceful_shutdown::{
    GracefulShutdownError, PartialShutdownError, SubsystemError, SubsystemHandle, Toplevel,
};

mod common;
use common::event::Event;

use std::error::Error;
use std::sync::Once;

static INIT: Once = Once::new();

/// Setup function that is only run once, even if called multiple times.
fn setup() {
    INIT.call_once(|| {
        // Init logging
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
async fn normal_shutdown() {
    setup();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;
            shutdown_token.shutdown();
        },
        async {
            let result: BoxedResult = toplevel
                .handle_shutdown_requests(Duration::from_millis(400))
                .await;
            assert!(result.is_ok());
        },
    );
}

#[tokio::test]
async fn shutdown_timeout_causes_error() {
    setup();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(400)).await;
        BoxedResult::Ok(())
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
            assert!(matches!(
                result,
                Err(GracefulShutdownError::ShutdownTimeout(_))
            ))
        },
    );
}

#[tokio::test]
async fn subsystem_finishes_with_success() {
    setup();

    let subsystem = |_| async { BoxedResult::Ok(()) };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let subsystem = |_| async { Err(anyhow!("Error!")) };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsys);
    let shutdown_token = toplevel.get_shutdown_token().clone();
    let result =
        tokio::spawn(toplevel.handle_shutdown_requests::<BoxedError>(Duration::from_millis(100)));

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
    setup();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.start("nested", nested_subsystem);
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();
    let result =
        tokio::spawn(toplevel.handle_shutdown_requests::<BoxedError>(Duration::from_millis(100)));

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
    setup();

    let nested_subsystem = |_subsys: SubsystemHandle| async move { Err(anyhow!("Error!")) };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start("nested", nested_subsystem);
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let nested_subsystem = |_subsys: SubsystemHandle| async move {
        panic!("Error!");
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start::<anyhow::Error, _, _>("nested", nested_subsystem);
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let (subsystem_should_stop, stop_subsystem) = Event::create();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsystem_should_stop.wait().await;
        subsys.request_shutdown();
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(300)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let (subsys_finished, set_subsys_finished) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested = |_: SubsystemHandle| async move {
        sleep(Duration::from_millis(100)).await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        subsys.start("Nested", nested);
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
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
    setup();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys3 = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(400)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsys2 = |_subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem2 panicked!")
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        subsys.start::<BoxedError, _, _>("Subsys2", subsys2);
        subsys.start::<BoxedError, _, _>("Subsys3", subsys3);
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem1 panicked!")
    };

    let result: BoxedResult = Toplevel::new()
        .start::<BoxedError, _, _>("subsys", subsys1)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;
    assert!(result.is_err());

    assert!(subsys_finished.get());
}

#[tokio::test]
async fn destroying_toplevel_cancels_subsystems() {
    setup();

    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys1 = move |_subsys: SubsystemHandle| async move {
        set_subsys_started();
        sleep(Duration::from_millis(100)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
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
    setup();

    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys2 = move |_subsys: SubsystemHandle| async move {
        set_subsys_started();
        sleep(Duration::from_millis(100)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |_subsys: SubsystemHandle| async move {
        Toplevel::new()
            .start("subsys2", subsys2)
            .handle_shutdown_requests::<BoxedError>(Duration::from_millis(100))
            .await
    };

    {
        let _result = Toplevel::new().start("subsys", subsys1);
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

#[tokio::test]
async fn partial_shutdown_request_stops_nested_subsystems() {
    setup();

    let (subsys1_started, set_subsys1_started) = Event::create();
    let (subsys1_finished, set_subsys1_finished) = Event::create();
    let (subsys2_started, set_subsys2_started) = Event::create();
    let (subsys2_finished, set_subsys2_finished) = Event::create();
    let (subsys3_started, set_subsys3_started) = Event::create();
    let (subsys3_finished, set_subsys3_finished) = Event::create();
    let (subsys1_shutdown_performed, set_subsys1_shutdown_performed) = Event::create();

    let subsys3 = move |subsys: SubsystemHandle| async move {
        set_subsys3_started();
        subsys.on_shutdown_requested().await;
        set_subsys3_finished();
        BoxedResult::Ok(())
    };
    let subsys2 = move |subsys: SubsystemHandle| async move {
        set_subsys2_started();
        subsys.start("subsys3", subsys3);
        subsys.on_shutdown_requested().await;
        set_subsys2_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        set_subsys1_started();
        let nested_subsys = subsys.start("subsys2", subsys2);
        sleep(Duration::from_millis(200)).await;
        subsys
            .perform_partial_shutdown(nested_subsys)
            .await
            .unwrap();
        set_subsys1_shutdown_performed();
        subsys.on_shutdown_requested().await;
        set_subsys1_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new();
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
                .start("subsys", subsys1)
                .handle_shutdown_requests(Duration::from_millis(500))
                .await;
            assert!(result.is_ok());
        },
        async {
            sleep(Duration::from_millis(300)).await;
            assert!(subsys1_started.get());
            assert!(subsys2_started.get());
            assert!(subsys3_started.get());
            assert!(!subsys1_finished.get());
            assert!(subsys2_finished.get());
            assert!(subsys3_finished.get());
            assert!(subsys1_shutdown_performed.get());
            shutdown_token.shutdown();
        }
    );
}

#[tokio::test]
async fn partial_shutdown_panic_gets_propagated_correctly() {
    setup();

    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = move |subsys: SubsystemHandle| async move {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        panic!("Nested panicked.");
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        let handle = subsys.start::<anyhow::Error, _, _>("nested", nested_subsys);
        sleep(Duration::from_millis(100)).await;
        let result = subsys.perform_partial_shutdown(handle).await;

        assert!(matches!(
            result.err(),
            Some(PartialShutdownError::SubsystemsFailed(_))
        ));
        assert!(nested_started.get());
        assert!(nested_finished.get());
        assert!(!subsys.local_shutdown_token().is_shutting_down());

        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let result: BoxedResult = Toplevel::new()
        .start("subsys", subsys1)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn partial_shutdown_error_gets_propagated_correctly() {
    setup();

    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = move |subsys: SubsystemHandle| async move {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        Err(anyhow!("nested failed."))
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        let handle = subsys.start("nested", nested_subsys);
        sleep(Duration::from_millis(100)).await;
        let result = subsys.perform_partial_shutdown(handle).await;

        assert!(matches!(
            result.err(),
            Some(PartialShutdownError::SubsystemsFailed(_))
        ));
        assert!(nested_started.get());
        assert!(nested_finished.get());
        assert!(!subsys.local_shutdown_token().is_shutting_down());

        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let result: BoxedResult = Toplevel::new()
        .start("subsys", subsys1)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn partial_shutdown_during_program_shutdown_causes_error() {
    setup();

    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = move |subsys: SubsystemHandle| async move {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        let handle = subsys.start("nested", nested_subsys);
        sleep(Duration::from_millis(100)).await;

        subsys.request_shutdown();
        sleep(Duration::from_millis(100)).await;
        let result = subsys.perform_partial_shutdown(handle).await;

        assert!(matches!(
            result.err(),
            Some(PartialShutdownError::AlreadyShuttingDown)
        ));

        sleep(Duration::from_millis(100)).await;

        assert!(nested_started.get());
        assert!(nested_finished.get());

        BoxedResult::Ok(())
    };

    let result: BoxedResult = Toplevel::new()
        .start("subsys", subsys1)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn partial_shutdown_on_wrong_parent_causes_error() {
    setup();

    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = move |subsys: SubsystemHandle| async move {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        let handle = subsys.start("nested", nested_subsys);

        sleep(Duration::from_millis(100)).await;

        let wrong_parent = |child_subsys: SubsystemHandle| async move {
            sleep(Duration::from_millis(100)).await;
            let result = child_subsys.perform_partial_shutdown(handle).await;
            assert!(matches!(
                result.err(),
                Some(PartialShutdownError::SubsystemNotFound)
            ));

            child_subsys.request_shutdown();
            sleep(Duration::from_millis(100)).await;

            assert!(nested_started.get());
            assert!(nested_finished.get());

            BoxedResult::Ok(())
        };

        subsys.start("wrong_parent", wrong_parent);
        subsys.on_shutdown_requested().await;

        BoxedResult::Ok(())
    };

    let result: BoxedResult = Toplevel::new()
        .start("subsys", subsys1)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn cloned_handles_can_spawn_nested_subsystems() {
    setup();

    let (toplevel_finished, set_toplevel_finished) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (nested1_finished, set_nested1_finished) = Event::create();
    let (nested2_finished, set_nested2_finished) = Event::create();

    let nested_subsystem1 = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_nested1_finished();
        BoxedResult::Ok(())
    };

    let nested_subsystem2 = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_nested2_finished();
        BoxedResult::Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        let subsys_clone = subsys.clone();
        subsys.start("nested1", nested_subsystem1);
        subsys_clone.start("nested2", nested_subsystem2);
        subsys_clone.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    tokio::join!(
        async {
            let result: BoxedResult = toplevel
                .handle_shutdown_requests(Duration::from_millis(200))
                .await;
            set_toplevel_finished();
            // Assert panic causes Error propagation to Toplevel
            assert!(result.is_ok());
        },
        async {
            // Assert that subsystems don't shut down prematurely
            sleep(Duration::from_millis(100)).await;
            assert!(!subsys_finished.get());
            assert!(!nested1_finished.get());
            assert!(!nested2_finished.get());
            assert!(!toplevel_finished.get());

            shutdown_token.shutdown();
            sleep(Duration::from_millis(100)).await;
            // Assert subsystems did shut down properly
            assert!(subsys_finished.get());
            assert!(nested1_finished.get());
            assert!(nested2_finished.get());
            assert!(toplevel_finished.get());
            assert!(shutdown_token.is_shutting_down());
        },
    );
}

#[tokio::test]
async fn subsystem_errors_get_propagated_to_user() {
    setup();

    let nested_subsystem1 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem panicked!");
    };

    let nested_subsystem2 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(100)).await;
        BoxedResult::Err("MyGreatError".into())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start::<anyhow::Error, _, _>("nested1", nested_subsystem1);
        subsys.start("nested2", nested_subsystem2);

        sleep(Duration::from_millis(100)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let result = toplevel
        .handle_shutdown_requests::<GracefulShutdownError>(Duration::from_millis(200))
        .await;

    if let Err(GracefulShutdownError::SubsystemsFailed(mut errors)) = result {
        assert_eq!(2, errors.len());

        errors.sort_by_key(|el| el.name().to_string());

        let mut iter = errors.into_iter();

        let el = iter.next().unwrap();
        assert!(matches!(el, SubsystemError::Panicked(_)));
        assert_eq!("subsys/nested1", el.name());

        let el = iter.next().unwrap();
        if let SubsystemError::Failed(name, e) = &el {
            assert_eq!("subsys/nested2", name);
            assert_eq!("MyGreatError", format!("{}", e));
        } else {
            assert!(false, "Incorrect error type!");
        }
        assert!(matches!(el, SubsystemError::Failed(_, _)));
        assert_eq!("subsys/nested2", el.name());
    } else {
        assert!(false, "Incorrect return value!");
    }
}

#[tokio::test]
async fn subsystem_errors_get_propagated_to_user_when_timeout() {
    setup();

    let nested_subsystem1 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem panicked!");
    };

    let nested_subsystem2 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(100)).await;
        BoxedResult::Err("MyGreatError".into())
    };

    let nested_subsystem3 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(10000)).await;
        Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start::<anyhow::Error, _, _>("nested1", nested_subsystem1);
        subsys.start("nested2", nested_subsystem2);
        subsys.start::<anyhow::Error, _, _>("nested3", nested_subsystem3);

        sleep(Duration::from_millis(100)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let result = toplevel
        .handle_shutdown_requests::<GracefulShutdownError>(Duration::from_millis(200))
        .await;

    if let Err(GracefulShutdownError::ShutdownTimeout(mut errors)) = result {
        assert_eq!(3, errors.len());

        errors.sort_by_key(|el| el.name().to_string());

        let mut iter = errors.into_iter();

        let el = iter.next().unwrap();
        assert!(matches!(el, SubsystemError::Panicked(_)));
        assert_eq!("subsys/nested1", el.name());

        let el = iter.next().unwrap();
        if let SubsystemError::Failed(name, e) = &el {
            assert_eq!("subsys/nested2", name);
            assert_eq!("MyGreatError", format!("{}", e));
        } else {
            assert!(false, "Incorrect error type!");
        }
        assert!(matches!(el, SubsystemError::Failed(_, _)));
        assert_eq!("subsys/nested2", el.name());

        let el = iter.next().unwrap();
        assert!(matches!(el, SubsystemError::Cancelled(_)));
        assert_eq!("subsys/nested3", el.name());
    } else {
        assert!(false, "Incorrect return value!");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_through_signal() {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    setup();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new().catch_signals();
    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;

            // Send SIGINT to ourselves.
            signal::kill(Pid::this(), Signal::SIGINT).unwrap();
        },
        async {
            let result: BoxedResult = toplevel
                .start("subsys", subsystem)
                .handle_shutdown_requests(Duration::from_millis(400))
                .await;
            assert!(result.is_ok());
        },
    );
}

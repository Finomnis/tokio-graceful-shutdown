// Required for test coverage
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use anyhow::anyhow;
use tokio::time::{sleep, timeout, Duration};
use tokio_graceful_shutdown::{
    errors::{GracefulShutdownError, SubsystemError, SubsystemJoinError},
    ErrorAction, IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel,
};
use tracing_test::traced_test;

pub mod common;
use common::Event;

use std::error::Error;

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
#[traced_test]
async fn normal_shutdown() {
    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(400))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
#[traced_test]
async fn use_subsystem_struct() {
    struct MySubsystem;

    #[async_trait::async_trait]
    impl IntoSubsystem<BoxedError> for MySubsystem {
        async fn run(self, subsys: SubsystemHandle) -> BoxedResult {
            subsys.on_shutdown_requested().await;
            sleep(Duration::from_millis(200)).await;
            BoxedResult::Ok(())
        }
    }

    let toplevel = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new(
            "subsys",
            MySubsystem {}.into_subsystem(),
        ));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(400))
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
#[traced_test]
async fn shutdown_timeout_causes_error() {
    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(400)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(200))
        .await;
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(GracefulShutdownError::ShutdownTimeout(_))
    ));
}

#[tokio::test]
#[traced_test]
async fn subsystem_finishes_with_success() {
    let subsystem = |_| async { BoxedResult::Ok(()) };
    let subsystem2 = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::<BoxedError>::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
        s.start(SubsystemBuilder::new("subsys2", subsystem2));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            shutdown_token.cancel();
            sleep(Duration::from_millis(200)).await;
            // Assert toplevel sucessfully gets stopped, nothing hangs
            assert!(toplevel_finished.get());
        },
    );
}

#[tokio::test]
#[traced_test]
async fn subsystem_finishes_with_error() {
    let subsystem = |_| async { Err(anyhow!("Error!")) };
    let subsystem2 = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::<BoxedError>::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
        s.start(SubsystemBuilder::new("subsys2", subsystem2));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            assert!(shutdown_token.is_cancelled());
        },
    );
}

#[tokio::test]
#[traced_test]
async fn subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::<BoxedError>::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();
    let result = tokio::spawn(toplevel.handle_shutdown_requests(Duration::from_millis(100)));

    sleep(Duration::from_millis(100)).await;
    assert!(!subsys_finished.get());

    shutdown_token.cancel();
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
#[traced_test]
async fn nested_subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.start(SubsystemBuilder::new("nested", nested_subsystem));
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();
    let result = tokio::spawn(toplevel.handle_shutdown_requests(Duration::from_millis(100)));

    sleep(Duration::from_millis(100)).await;
    assert!(!subsys_finished.get());

    shutdown_token.cancel();
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
#[traced_test]
async fn nested_subsystem_error_propagates() {
    let nested_subsystem = |_subsys: SubsystemHandle| async move { Err(anyhow!("Error!")) };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start(SubsystemBuilder::new("nested", nested_subsystem));
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            assert!(shutdown_token.is_cancelled());
        },
    );
}

#[tokio::test]
#[traced_test]
async fn panic_gets_handled_correctly() {
    let nested_subsystem = |_subsys: SubsystemHandle| async move {
        panic!("Error!");
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested", nested_subsystem));
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            assert!(shutdown_token.is_cancelled());
        },
    );
}

#[tokio::test]
#[traced_test]
async fn subsystem_can_request_shutdown() {
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

    let toplevel = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            assert!(!shutdown_token.is_cancelled());

            stop_subsystem();
            sleep(Duration::from_millis(200)).await;

            // Assert request_shutdown() causes a shutdown
            assert!(toplevel_finished.get());
            assert!(subsys_finished.get());
            assert!(shutdown_token.is_cancelled());
        },
    );
}

#[tokio::test]
#[traced_test]
async fn shutdown_timeout_causes_cancellation() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(300)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            assert!(!shutdown_token.is_cancelled());

            shutdown_token.cancel();
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
#[traced_test]
async fn spawning_task_during_shutdown_causes_task_to_be_cancelled() {
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested = |subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(100)).await;
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        subsys.start(SubsystemBuilder::new("Nested", nested));
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

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
            assert!(!shutdown_token.is_cancelled());
            assert!(!nested_finished.get());

            shutdown_token.cancel();
            timeout(Duration::from_millis(300), toplevel_finished.wait())
                .await
                .unwrap();

            assert!(subsys_finished.get());
            assert!(nested_finished.get());
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
        BoxedResult::Ok(())
    };

    let subsys2 = |_subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem2 panicked!")
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        subsys.start::<BoxedError, _, _>(SubsystemBuilder::new("Subsys2", subsys2));
        subsys.start::<BoxedError, _, _>(SubsystemBuilder::new("Subsys3", subsys3));
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem1 panicked!")
    };

    let result = Toplevel::new(|s| async move {
        s.start::<BoxedError, _, _>(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await;
    assert!(result.is_err());

    assert!(subsys_finished.get());
}

#[tokio::test]
#[traced_test]
async fn destroying_toplevel_cancels_subsystems() {
    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys1 = move |_subsys: SubsystemHandle| async move {
        set_subsys_started();
        sleep(Duration::from_millis(200)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    {
        let _result = Toplevel::new(|s| async move {
            s.start(SubsystemBuilder::new("subsys", subsys1));
        });
        sleep(Duration::from_millis(100)).await;
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

#[tokio::test]
#[traced_test]
async fn shutdown_triggers_if_all_tasks_ended() {
    let nested_subsys = move |_subsys: SubsystemHandle| async move { BoxedResult::Ok(()) };

    let subsys = move |subsys: SubsystemHandle| async move {
        subsys.start(SubsystemBuilder::new("nested", nested_subsys));
        BoxedResult::Ok(())
    };

    tokio::time::timeout(
        Duration::from_millis(100),
        Toplevel::new(move |s| async move {
            s.start(SubsystemBuilder::new("subsys1", subsys));
            s.start(SubsystemBuilder::new("subsys2", subsys));
        })
        .handle_shutdown_requests(Duration::from_millis(100)),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
#[traced_test]
async fn shutdown_triggers_if_no_task_exists() {
    tokio::time::timeout(
        Duration::from_millis(100),
        Toplevel::<BoxedError>::new(|_| async {})
            .handle_shutdown_requests(Duration::from_millis(100)),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
#[traced_test]
async fn destroying_toplevel_cancels_nested_toplevel_subsystems() {
    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys2 = move |_subsys: SubsystemHandle| async move {
        set_subsys_started();
        sleep(Duration::from_millis(100)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |_subsys: SubsystemHandle| async move {
        Toplevel::new(|s| async move {
            s.start(SubsystemBuilder::new("subsys2", subsys2));
        })
        .handle_shutdown_requests(Duration::from_millis(100))
        .await
    };

    {
        let _result = Toplevel::new(|s| async move {
            s.start(SubsystemBuilder::new("subsys", subsys1));
        });
        sleep(Duration::from_millis(50)).await;
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

#[tokio::test]
#[traced_test]
async fn partial_shutdown_request_stops_nested_subsystems() {
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
        subsys.start(SubsystemBuilder::new("subsys3", subsys3));
        subsys.on_shutdown_requested().await;
        set_subsys2_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        set_subsys1_started();
        let nested_subsys = subsys.start(SubsystemBuilder::new("subsys2", subsys2));
        sleep(Duration::from_millis(200)).await;
        nested_subsys.change_failure_action(ErrorAction::CatchAndLocalShutdown);
        nested_subsys.change_panic_action(ErrorAction::CatchAndLocalShutdown);
        nested_subsys.initiate_shutdown();
        nested_subsys.join().await.unwrap();
        set_subsys1_shutdown_performed();
        subsys.on_shutdown_requested().await;
        set_subsys1_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

    tokio::join!(
        async {
            let result = toplevel
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
            shutdown_token.cancel();
        }
    );
}

#[tokio::test]
#[traced_test]
async fn partial_shutdown_panic_gets_propagated_correctly() {
    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = move |subsys: SubsystemHandle| async move {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        panic!("Nested panicked.");
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        let handle = subsys.start::<anyhow::Error, _, _>(
            SubsystemBuilder::new("nested", nested_subsys)
                .on_failure(ErrorAction::CatchAndLocalShutdown)
                .on_panic(ErrorAction::CatchAndLocalShutdown),
        );
        sleep(Duration::from_millis(100)).await;
        handle.initiate_shutdown();
        let result = handle.join().await;

        assert!(matches!(
            result.err(),
            Some(SubsystemJoinError::SubsystemsFailed(_))
        ));
        assert!(nested_started.get());
        assert!(nested_finished.get());
        assert!(!subsys.is_shutdown_requested());

        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let result = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
#[traced_test]
async fn partial_shutdown_error_gets_propagated_correctly() {
    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = move |subsys: SubsystemHandle| async move {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        Err(anyhow!("nested failed."))
    };

    let subsys1 = move |subsys: SubsystemHandle| async move {
        let handle = subsys.start(
            SubsystemBuilder::new("nested", nested_subsys)
                .on_failure(ErrorAction::CatchAndLocalShutdown)
                .on_panic(ErrorAction::CatchAndLocalShutdown),
        );
        sleep(Duration::from_millis(100)).await;
        handle.initiate_shutdown();
        let result = handle.join().await;

        assert!(matches!(
            result.err(),
            Some(SubsystemJoinError::SubsystemsFailed(_))
        ));
        assert!(nested_started.get());
        assert!(nested_finished.get());
        assert!(!subsys.is_shutdown_requested());

        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let result = Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
#[traced_test]
async fn subsystem_errors_get_propagated_to_user() {
    let nested_subsystem1 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem panicked!");
    };

    let nested_subsystem2 = |_: SubsystemHandle| async {
        sleep(Duration::from_millis(100)).await;
        BoxedResult::Err("MyGreatError".into())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested1", nested_subsystem1));
        subsys.start(SubsystemBuilder::new("nested2", nested_subsystem2));

        sleep(Duration::from_millis(100)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(200))
        .await;

    if let Err(GracefulShutdownError::SubsystemsFailed(mut errors)) = result {
        assert_eq!(2, errors.len());

        errors.sort_by_key(|el| el.name().to_string());

        let mut iter = errors.iter();

        let el = iter.next().unwrap();
        assert!(matches!(el, SubsystemError::Panicked(_)));
        assert_eq!("/subsys/nested1", el.name());

        let el = iter.next().unwrap();
        if let SubsystemError::Failed(name, e) = &el {
            assert_eq!("/subsys/nested2", name.as_ref());
            assert_eq!("MyGreatError", format!("{}", e));
        } else {
            panic!("Incorrect error type!");
        }
        assert!(matches!(el, SubsystemError::Failed(_, _)));
        assert_eq!("/subsys/nested2", el.name());
    } else {
        panic!("Incorrect return value!");
    }
}

#[tokio::test]
#[traced_test]
async fn subsystem_errors_get_propagated_to_user_when_timeout() {
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
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested1", nested_subsystem1));
        subsys.start(SubsystemBuilder::new("nested2", nested_subsystem2));
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested3", nested_subsystem3));

        sleep(Duration::from_millis(100)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(200))
        .await;

    if let Err(GracefulShutdownError::ShutdownTimeout(mut errors)) = result {
        assert_eq!(2, errors.len());

        errors.sort_by_key(|el| el.name().to_string());

        let mut iter = errors.iter();

        let el = iter.next().unwrap();
        assert!(matches!(el, SubsystemError::Panicked(_)));
        assert_eq!("/subsys/nested1", el.name());

        let el = iter.next().unwrap();
        if let SubsystemError::Failed(name, e) = &el {
            assert_eq!("/subsys/nested2", name.as_ref());
            assert_eq!("MyGreatError", format!("{}", e));
        } else {
            panic!("Incorrect error type!");
        }
        assert!(matches!(el, SubsystemError::Failed(_, _)));
        assert_eq!("/subsys/nested2", el.name());

        assert!(iter.next().is_none());
    } else {
        panic!("Incorrect return value!");
    }
}

#[tokio::test]
#[traced_test]
async fn is_shutdown_requested_works_as_intended() {
    let subsys1 = move |subsys: SubsystemHandle| async move {
        assert!(!subsys.is_shutdown_requested());
        subsys.request_shutdown();
        assert!(subsys.is_shutdown_requested());
        BoxedResult::Ok(())
    };

    Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

#[cfg(unix)]
#[tokio::test]
#[traced_test]
async fn shutdown_through_signal() {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;
    use tokio_graceful_shutdown::FutureExt;

    let subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Ok(())
    };

    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;

            // Send SIGINT to ourselves.
            signal::kill(Pid::this(), Signal::SIGINT).unwrap();
        },
        async {
            let result = Toplevel::new(move |s| async move {
                s.start(SubsystemBuilder::new("subsys", subsystem));
                assert!(sleep(Duration::from_millis(1000))
                    .cancel_on_shutdown(&s)
                    .await
                    .is_err());
                assert!(s.is_shutdown_requested());
            })
            .catch_signals()
            .handle_shutdown_requests(Duration::from_millis(400))
            .await;
            assert!(result.is_ok());
        },
    );
}

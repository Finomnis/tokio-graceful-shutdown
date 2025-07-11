use anyhow::anyhow;
use tokio::time::{Duration, sleep, timeout};
use tokio_graceful_shutdown::{
    ErrorAction, IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel,
    errors::{GracefulShutdownError, SubsystemError, SubsystemJoinError},
};
use tracing_test::traced_test;

pub mod common;
use common::Event;

use common::{BoxedError, BoxedResult};

#[tokio::test(start_paused = true)]
#[traced_test]
async fn normal_shutdown() {
    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
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

    let toplevel = Toplevel::new(async |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn shutdown_timeout_causes_error() {
    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(400)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_finishes_with_success() {
    let subsystem = async |_| BoxedResult::Ok(());
    let subsystem2 = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::<BoxedError>::new(async move |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_finishes_with_error() {
    let subsystem = async |_| Err(anyhow!("Error!"));
    let subsystem2 = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::<BoxedError>::new(async move |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::<BoxedError>::new(async |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn nested_subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let nested_subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsystem = async |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested", nested_subsystem));
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn nested_subsystem_error_propagates() {
    let nested_subsystem = async |_subsys: SubsystemHandle| Err(anyhow!("Error!"));

    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested", nested_subsystem));
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(async move |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn panic_gets_handled_correctly() {
    let nested_subsystem = async |_subsys: SubsystemHandle| {
        panic!("Error!");
    };

    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested", nested_subsystem));
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(async move |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_can_request_shutdown() {
    let (subsystem_should_stop, stop_subsystem) = Event::create();

    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = async move |subsys: SubsystemHandle| {
        subsystem_should_stop.wait().await;
        subsys.request_shutdown();
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(async |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn shutdown_timeout_causes_cancellation() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(300)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(async |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn spawning_task_during_shutdown_causes_task_to_be_cancelled() {
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested = async |subsys: SubsystemHandle| {
        sleep(Duration::from_millis(100)).await;
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(100)).await;
        subsys.start(SubsystemBuilder::new("Nested", nested));
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let toplevel = Toplevel::new(async |s| {
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

    let subsys3 = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(40)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsys2 = async |_subsys: SubsystemHandle| {
        sleep(Duration::from_millis(10)).await;
        panic!("Subsystem2 panicked!")
    };

    let subsys1 = async move |subsys: SubsystemHandle| {
        subsys.start::<BoxedError, _, _>(SubsystemBuilder::new("Subsys2", subsys2));
        subsys.start::<BoxedError, _, _>(SubsystemBuilder::new("Subsys3", subsys3));
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(10)).await;
        panic!("Subsystem1 panicked!")
    };

    let result = Toplevel::new(async |s| {
        s.start::<BoxedError, _, _>(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(50))
    .await;
    assert!(result.is_err());

    assert!(subsys_finished.get());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn destroying_toplevel_cancels_subsystems() {
    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys1 = async move |_subsys: SubsystemHandle| {
        set_subsys_started();
        sleep(Duration::from_millis(200)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    {
        let _result = Toplevel::new(async |s| {
            s.start(SubsystemBuilder::new("subsys", subsys1));
        });
        sleep(Duration::from_millis(100)).await;
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn shutdown_triggers_if_all_tasks_ended() {
    let nested_subsys = async move |_subsys: SubsystemHandle| BoxedResult::Ok(());

    let subsys = async move |subsys: SubsystemHandle| {
        subsys.start(SubsystemBuilder::new("nested", nested_subsys));
        BoxedResult::Ok(())
    };

    tokio::time::timeout(
        Duration::from_millis(100),
        Toplevel::new(async move |s| {
            s.start(SubsystemBuilder::new("subsys1", subsys));
            s.start(SubsystemBuilder::new("subsys2", subsys));
        })
        .handle_shutdown_requests(Duration::from_millis(100)),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn shutdown_triggers_if_no_task_exists() {
    tokio::time::timeout(
        Duration::from_millis(100),
        Toplevel::<BoxedError>::new(async |_| {})
            .handle_shutdown_requests(Duration::from_millis(100)),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn destroying_toplevel_cancels_nested_toplevel_subsystems() {
    let (subsys_started, set_subsys_started) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys2 = async move |_subsys: SubsystemHandle| {
        set_subsys_started();
        sleep(Duration::from_millis(100)).await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = async move |_subsys: SubsystemHandle| {
        Toplevel::new(async |s| {
            s.start(SubsystemBuilder::new("subsys2", subsys2));
        })
        .handle_shutdown_requests(Duration::from_millis(100))
        .await
    };

    {
        let _result = Toplevel::new(async |s| {
            s.start(SubsystemBuilder::new("subsys", subsys1));
        });
        sleep(Duration::from_millis(50)).await;
    }

    sleep(Duration::from_millis(300)).await;
    assert!(subsys_started.get());
    assert!(!subsys_finished.get());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn partial_shutdown_request_stops_nested_subsystems() {
    let (subsys1_started, set_subsys1_started) = Event::create();
    let (subsys1_finished, set_subsys1_finished) = Event::create();
    let (subsys2_started, set_subsys2_started) = Event::create();
    let (subsys2_finished, set_subsys2_finished) = Event::create();
    let (subsys3_started, set_subsys3_started) = Event::create();
    let (subsys3_finished, set_subsys3_finished) = Event::create();
    let (subsys1_shutdown_performed, set_subsys1_shutdown_performed) = Event::create();

    let subsys3 = async move |subsys: SubsystemHandle| {
        set_subsys3_started();
        subsys.on_shutdown_requested().await;
        set_subsys3_finished();
        BoxedResult::Ok(())
    };
    let subsys2 = async move |subsys: SubsystemHandle| {
        set_subsys2_started();
        subsys.start(SubsystemBuilder::new("subsys3", subsys3));
        subsys.on_shutdown_requested().await;
        set_subsys2_finished();
        BoxedResult::Ok(())
    };

    let subsys1 = async move |subsys: SubsystemHandle| {
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

    let toplevel = Toplevel::new(async move |s| {
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn partial_shutdown_panic_gets_propagated_correctly() {
    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = async move |subsys: SubsystemHandle| {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        panic!("Nested panicked.");
    };

    let subsys1 = async move |subsys: SubsystemHandle| {
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

    let result = Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await;

    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn partial_shutdown_error_gets_propagated_correctly() {
    let (nested_started, set_nested_started) = Event::create();
    let (nested_finished, set_nested_finished) = Event::create();

    let nested_subsys = async move |subsys: SubsystemHandle| {
        set_nested_started();
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        Err(anyhow!("nested failed."))
    };

    let subsys1 = async move |subsys: SubsystemHandle| {
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

    let result = Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("subsys", subsys1));
    })
    .handle_shutdown_requests(Duration::from_millis(500))
    .await;

    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_errors_get_propagated_to_user() {
    let nested_subsystem1 = async |_: SubsystemHandle| {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem panicked!");
    };

    let nested_subsystem2 = async |_: SubsystemHandle| {
        sleep(Duration::from_millis(100)).await;
        BoxedResult::Err("MyGreatError".into())
    };

    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested1", nested_subsystem1));
        subsys.start(SubsystemBuilder::new("nested2", nested_subsystem2));

        sleep(Duration::from_millis(100)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
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
            assert_eq!("MyGreatError", format!("{e}"));
        } else {
            panic!("Incorrect error type!");
        }
        assert!(matches!(el, SubsystemError::Failed(_, _)));
        assert_eq!("/subsys/nested2", el.name());
    } else {
        panic!("Incorrect return value!");
    }
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn subsystem_errors_get_propagated_to_user_when_timeout() {
    let nested_subsystem1 = async |_: SubsystemHandle| {
        sleep(Duration::from_millis(100)).await;
        panic!("Subsystem panicked!");
    };

    let nested_subsystem2 = async |_: SubsystemHandle| {
        sleep(Duration::from_millis(100)).await;
        BoxedResult::Err("MyGreatError".into())
    };

    let nested_subsystem3 = async |_: SubsystemHandle| {
        sleep(Duration::from_millis(10000)).await;
        Ok(())
    };

    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested1", nested_subsystem1));
        subsys.start(SubsystemBuilder::new("nested2", nested_subsystem2));
        subsys.start::<anyhow::Error, _, _>(SubsystemBuilder::new("nested3", nested_subsystem3));

        sleep(Duration::from_millis(100)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
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
            assert_eq!("MyGreatError", format!("{e}"));
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

#[tokio::test(start_paused = true)]
#[traced_test]
async fn is_shutdown_requested_works_as_intended() {
    let subsys1 = async move |subsys: SubsystemHandle| {
        assert!(!subsys.is_shutdown_requested());
        subsys.request_shutdown();
        assert!(subsys.is_shutdown_requested());
        BoxedResult::Ok(())
    };

    Toplevel::new(async move |s| {
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

    let subsystem = async |subsys: SubsystemHandle| {
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
async fn access_name_from_within_subsystem() {
    let subsys_nested = async move |subsys: SubsystemHandle| {
        assert_eq!("/subsys_top/subsys_nested", subsys.name());
        BoxedResult::Ok(())
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        assert_eq!("/subsys_top", subsys.name());
        subsys.start(SubsystemBuilder::new("subsys_nested", subsys_nested));
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
async fn query_subsystem_alive() {
    // Diagram:
    //
    // top (2.5s lifetime)
    //   \
    //    nested (1s lifetime)

    let subsys_nested = async move |_: SubsystemHandle| {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        BoxedResult::Ok(())
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("subsys_nested", subsys_nested));
        assert!(!nested.is_finished_shallow());
        assert!(!nested.is_finished());

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(!nested.is_finished_shallow());
        assert!(!nested.is_finished());

        tokio::time::sleep(Duration::from_millis(2000)).await;
        assert!(nested.is_finished_shallow());
        assert!(nested.is_finished());

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
async fn query_multidepth_subsystem_alive() {
    // Diagram:
    //
    // top (2.2s lifetime)
    //   \
    //    d1 (1s lifetime)
    //     \
    //      d2 (2s lifetime)
    //
    // We want to ensure that root_alive() only lasts for the duration of d1,
    // but recursive_alive() lasts for the entire duration of d2.

    let subsys_nested_d2 = async move |_: SubsystemHandle| {
        tokio::time::sleep(Duration::from_millis(2000)).await;
        BoxedResult::Ok(())
    };

    let subsys_nested_d1 = async move |subsys: SubsystemHandle| {
        let _nested = subsys.start(SubsystemBuilder::new("d2", subsys_nested_d2));
        tokio::time::sleep(Duration::from_millis(1000)).await;

        BoxedResult::Ok(())
    };

    let subsys_top = async move |subsys: SubsystemHandle| {
        let nested = subsys.start(SubsystemBuilder::new("d1", subsys_nested_d1));
        assert!(!nested.is_finished_shallow());
        assert!(!nested.is_finished());

        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(nested.is_finished_shallow());
        assert!(!nested.is_finished());

        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(nested.is_finished_shallow());
        assert!(nested.is_finished());

        BoxedResult::Ok(())
    };

    Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys_top", subsys_top));
    })
    .handle_shutdown_requests(Duration::from_millis(100))
    .await
    .unwrap();
}

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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(400)).await;
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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(200)).await;
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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
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
    let result = tokio::spawn(toplevel.wait_for_shutdown(Duration::from_millis(100)));

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
    let result = tokio::spawn(toplevel.wait_for_shutdown(Duration::from_millis(100)));

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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
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
            let result = toplevel.wait_for_shutdown(Duration::from_millis(200)).await;
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

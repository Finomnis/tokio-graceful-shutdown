use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::anyhow;
use tokio::time::{sleep, timeout, Duration};
use tokio_graceful_shutdown::subsys::LambdaSubsystem;
use tokio_graceful_shutdown::Toplevel;

mod common;
use common::event::Event;
use common::immediate::ImmediateSubsystem;
use common::slow_shutdown::SlowShutdownSubsystem;

#[tokio::test]
async fn normal_shutdown() {
    let subsystem = SlowShutdownSubsystem::new(Duration::from_millis(500));

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    let runner = toplevel.wait_for_shutdown(Duration::from_millis(1000));

    let tester = async {
        sleep(Duration::from_millis(200)).await;
        shutdown_token.shutdown();
    };

    let (result, ()) = tokio::join!(runner, tester);
    assert!(result.is_ok());
}

#[tokio::test]
async fn shutdown_timeout() {
    let subsystem = SlowShutdownSubsystem::new(Duration::from_millis(1000));

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    let runner = toplevel.wait_for_shutdown(Duration::from_millis(500));

    let tester = async {
        sleep(Duration::from_millis(200)).await;
        shutdown_token.shutdown();
    };

    let (result, ()) = tokio::join!(runner, tester);
    assert!(result.is_err());
}

#[tokio::test]
async fn subsystem_finishes_with_success() {
    let subsystem = ImmediateSubsystem::new();

    let toplevel_finished = AtomicBool::new(false);

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();
    let runner = async {
        let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
        toplevel_finished.store(true, Ordering::SeqCst);
        result
    };

    let tester = async {
        sleep(Duration::from_millis(200)).await;
        assert!(!toplevel_finished.load(Ordering::SeqCst));
        shutdown_token.shutdown();
        sleep(Duration::from_millis(200)).await;
        assert!(toplevel_finished.load(Ordering::SeqCst));
    };

    let (result, ()) = tokio::join!(runner, tester);
    assert!(result.is_ok());
}

#[tokio::test]
async fn subsystem_finishes_with_error() {
    let subsystem = ImmediateSubsystem::new().return_value(Err(anyhow!("Error!")));

    let toplevel_finished = AtomicBool::new(false);

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();
    let runner = async {
        let result = toplevel.wait_for_shutdown(Duration::from_millis(100)).await;
        toplevel_finished.store(true, Ordering::SeqCst);
        result
    };

    let tester = async {
        sleep(Duration::from_millis(200)).await;
        assert!(toplevel_finished.load(Ordering::SeqCst));
        shutdown_token.is_shutting_down();
    };

    let (result, ()) = tokio::join!(runner, tester);
    assert!(result.is_err());
}

#[tokio::test]
async fn lambda_subsystem_receives_shutdown() {
    let (subsys_finished, set_subsys_finished) = Event::create();

    let subsys = LambdaSubsystem::new(|subsys| async move {
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        Ok(())
    });

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

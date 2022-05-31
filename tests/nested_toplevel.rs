use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{GracefulShutdownError, SubsystemHandle, Toplevel};

pub mod common;
use common::event::Event;
use common::setup;

use std::error::Error;

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

/*
- nested toplevel shuts down on external shutdown
- errors/panics/shutdown requests do not get propagated out
- global_shutdown does get propagated out
*/

#[tokio::test]
async fn nested_toplevel_shuts_down_when_requested() {
    setup();

    let (nested_finished, set_nested_finished) = Event::create();
    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = |subsys: SubsystemHandle| async move {
        let nested_toplevel = Toplevel::nested(&subsys);
        nested_toplevel
            .start("nested", nested_subsystem)
            .handle_shutdown_requests::<GracefulShutdownError>(Duration::from_millis(100))
            .await?;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::<BoxedError>::new().start("subsys", subsystem);
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
            assert!(!nested_finished.get());
            shutdown_token.shutdown();
            sleep(Duration::from_millis(200)).await;
            // Assert toplevel sucessfully gets stopped, nothing hangs
            assert!(toplevel_finished.get());
            assert!(nested_finished.get());
        },
    );
}

#[tokio::test]
async fn nested_toplevel_errors_do_not_get_propagated_up() {
    setup();

    let (nested_finished, set_nested_finished) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let nested_error_subsystem = |_subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Err("Error from nested subsystem".into())
    };
    let nested_panic_subsystem = |_subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(200)).await;
        panic!("Panic from nested subsystem");
    };

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        let nested_toplevel = Toplevel::nested(&subsys);
        let result = nested_toplevel
            .start("nested", nested_subsystem)
            .start::<BoxedError, _, _>("nested_panic", nested_panic_subsystem)
            .start("nested_error", nested_error_subsystem)
            .handle_shutdown_requests::<GracefulShutdownError>(Duration::from_millis(100))
            .await;
        assert!(result.is_err());
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::<BoxedError>::new().start("subsys", subsystem);

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
            sleep(Duration::from_millis(100)).await;
            // Assert Ok(()) doesn't cause a shutdown
            assert!(!toplevel_finished.get());
            assert!(!nested_finished.get());
            assert!(!subsys_finished.get());
            sleep(Duration::from_millis(200)).await;
            // Assert toplevel sucessfully gets stopped, nothing hangs
            assert!(toplevel_finished.get());
            assert!(nested_finished.get());
            assert!(subsys_finished.get());
        },
    );
}

#[tokio::test]
async fn nested_toplevel_local_shutdown_does_not_get_propagated_up() {
    setup();

    let (nested_finished, set_nested_finished) = Event::create();
    let (nested_toplevel_finished, set_nested_toplevel_finished) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let nested_shutdown_subsystem = |subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(200)).await;
        subsys.request_shutdown();
        BoxedResult::Ok(())
    };

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        let nested_toplevel = Toplevel::nested(&subsys);
        let result = nested_toplevel
            .start("nested", nested_subsystem)
            .start("nested_shutdown", nested_shutdown_subsystem)
            .handle_shutdown_requests::<GracefulShutdownError>(Duration::from_millis(100))
            .await;
        assert!(result.is_ok());
        set_nested_toplevel_finished();
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::<BoxedError>::new().start("subsys", subsystem);
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
            sleep(Duration::from_millis(100)).await;
            // Assert Ok(()) doesn't cause a shutdown
            assert!(!toplevel_finished.get());
            assert!(!nested_finished.get());
            assert!(!nested_toplevel_finished.get());
            assert!(!subsys_finished.get());
            sleep(Duration::from_millis(200)).await;
            // Assert toplevel sucessfully gets stopped, nothing hangs
            assert!(!toplevel_finished.get());
            assert!(nested_finished.get());
            assert!(nested_toplevel_finished.get());
            assert!(!subsys_finished.get());
            shutdown_token.shutdown();
            sleep(Duration::from_millis(200)).await;
            assert!(toplevel_finished.get());
            assert!(nested_finished.get());
            assert!(nested_toplevel_finished.get());
            assert!(subsys_finished.get());
        },
    );
}

#[tokio::test]
async fn nested_toplevel_global_shutdown_does_get_propagated_up() {
    setup();

    let (nested_finished, set_nested_finished) = Event::create();
    let (nested_toplevel_finished, set_nested_toplevel_finished) = Event::create();
    let (subsys_finished, set_subsys_finished) = Event::create();
    let (toplevel_finished, set_toplevel_finished) = Event::create();

    let nested_shutdown_subsystem = |subsys: SubsystemHandle| async move {
        sleep(Duration::from_millis(200)).await;
        subsys.request_global_shutdown();
        BoxedResult::Ok(())
    };

    let nested_subsystem = |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;
        set_nested_finished();
        BoxedResult::Ok(())
    };

    let subsystem = move |subsys: SubsystemHandle| async move {
        let nested_toplevel = Toplevel::nested(&subsys);
        let result = nested_toplevel
            .start("nested", nested_subsystem)
            .start("nested_shutdown", nested_shutdown_subsystem)
            .handle_shutdown_requests::<GracefulShutdownError>(Duration::from_millis(100))
            .await;
        assert!(result.is_ok());
        set_nested_toplevel_finished();
        subsys.on_shutdown_requested().await;
        set_subsys_finished();
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::<BoxedError>::new().start("subsys", subsystem);

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
            sleep(Duration::from_millis(100)).await;
            // Assert Ok(()) doesn't cause a shutdown
            assert!(!toplevel_finished.get());
            assert!(!nested_finished.get());
            assert!(!nested_toplevel_finished.get());
            assert!(!subsys_finished.get());
            sleep(Duration::from_millis(200)).await;
            // Assert toplevel sucessfully gets stopped, nothing hangs
            assert!(toplevel_finished.get());
            assert!(nested_finished.get());
            assert!(nested_toplevel_finished.get());
            assert!(subsys_finished.get());
        },
    );
}

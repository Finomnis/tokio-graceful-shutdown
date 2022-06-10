use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{errors::CancelledByShutdown, FutureExt, SubsystemHandle, Toplevel};

pub mod common;
use common::setup;

use std::error::Error;

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
async fn cancel_on_shutdown_propagates_result() {
    setup();

    let subsystem1 = |subsys: SubsystemHandle| async move {
        let compute_value = async {
            sleep(Duration::from_millis(10)).await;
            42
        };

        let value = compute_value.cancel_on_shutdown(&subsys).await;

        assert_eq!(value.ok(), Some(42));

        BoxedResult::Ok(())
    };

    let subsystem2 = |subsys: SubsystemHandle| async move {
        async fn compute_value() -> i32 {
            sleep(Duration::from_millis(10)).await;
            42
        }

        let value = compute_value().cancel_on_shutdown(&subsys).await;

        assert_eq!(value.ok(), Some(42));

        BoxedResult::Ok(())
    };

    let result = Toplevel::<BoxedError>::new()
        .start("subsys1", subsystem1)
        .start("subsys2", subsystem2)
        .handle_shutdown_requests(Duration::from_millis(200))
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn cancel_on_shutdown_cancels_on_shutdown() {
    setup();

    let subsystem = |subsys: SubsystemHandle| async move {
        async fn compute_value(subsys: SubsystemHandle) -> i32 {
            sleep(Duration::from_millis(100)).await;
            subsys.request_shutdown();
            sleep(Duration::from_millis(100)).await;
            42
        }

        let value = compute_value(subsys.clone())
            .cancel_on_shutdown(&subsys)
            .await;

        assert!(matches!(value, Err(CancelledByShutdown)));

        BoxedResult::Ok(())
    };

    let result = Toplevel::<BoxedError>::new()
        .start("subsys", subsystem)
        .handle_shutdown_requests(Duration::from_millis(200))
        .await;

    assert!(result.is_ok());
}

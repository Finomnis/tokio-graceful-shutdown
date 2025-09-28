mod common;
use tokio::time::{Duration, sleep};
use tokio_graceful_shutdown::{
    FutureExt, SubsystemBuilder, SubsystemHandle, Toplevel, errors::CancelledByShutdown,
};
use tracing_test::traced_test;

use common::{BoxedError, BoxedResult};

#[tokio::test(start_paused = true)]
#[traced_test]
async fn cancel_on_shutdown_propagates_result() {
    let subsystem1 = async |subsys: &mut SubsystemHandle| {
        let compute_value = async {
            sleep(Duration::from_millis(10)).await;
            42
        };

        let value = compute_value.cancel_on_shutdown(subsys).await;

        assert_eq!(value.ok(), Some(42));

        BoxedResult::Ok(())
    };

    let subsystem2 = async |subsys: &mut SubsystemHandle| {
        async fn compute_value() -> i32 {
            sleep(Duration::from_millis(10)).await;
            42
        }

        let value = compute_value().cancel_on_shutdown(subsys).await;

        assert_eq!(value.ok(), Some(42));

        BoxedResult::Ok(())
    };

    let result = Toplevel::<BoxedError>::new(async move |s: &mut SubsystemHandle| {
        s.start(SubsystemBuilder::new("subsys1", subsystem1));
        s.start(SubsystemBuilder::new("subsys2", subsystem2));
    })
    .handle_shutdown_requests(Duration::from_millis(200))
    .await;

    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn cancel_on_shutdown_cancels_on_shutdown() {
    let subsystem = async |subsys: &mut SubsystemHandle| {
        async fn compute_value(subsys: &SubsystemHandle) -> i32 {
            sleep(Duration::from_millis(100)).await;
            subsys.request_shutdown();
            sleep(Duration::from_millis(100)).await;
            42
        }

        let value = compute_value(subsys).cancel_on_shutdown(subsys).await;

        assert!(matches!(value, Err(CancelledByShutdown)));

        BoxedResult::Ok(())
    };

    let result = Toplevel::<BoxedError>::new(async move |s: &mut SubsystemHandle| {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    })
    .handle_shutdown_requests(Duration::from_millis(200))
    .await;

    assert!(result.is_ok());
}

use tokio::time::{Duration, sleep, timeout};
use tracing_test::traced_test;

use super::*;

#[tokio::test(start_paused = true)]
#[traced_test]
async fn recursive_cancellation() {
    let root_handle = root_handle::<BoxedError>(CancellationToken::new(), |_| {});

    let (drop_sender, mut drop_receiver) = tokio::sync::mpsc::channel::<()>(1);

    root_handle.start(SubsystemBuilder::new(
        "",
        async move |_: &mut SubsystemHandle<BoxedError>| {
            drop_sender.send(()).await.unwrap();
            std::future::pending::<Result<(), BoxedError>>().await
        },
    ));

    // Make sure we are executing the subsystem
    let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
        .await
        .unwrap();
    assert!(recv_result.is_some());

    drop(root_handle);

    // Make sure the subsystem got cancelled
    let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
        .await
        .unwrap();
    assert!(recv_result.is_none());
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn recursive_cancellation_2() {
    let root_handle = root_handle(CancellationToken::new(), |_| {});

    let (drop_sender, mut drop_receiver) = tokio::sync::mpsc::channel::<()>(1);

    let subsys2 = async move |_: &mut SubsystemHandle| {
        drop_sender.send(()).await.unwrap();
        std::future::pending::<Result<(), BoxedError>>().await
    };

    let subsys = async |x: &mut SubsystemHandle| {
        x.start(SubsystemBuilder::new("", subsys2));

        Result::<(), BoxedError>::Ok(())
    };

    root_handle.start(SubsystemBuilder::new("", subsys));

    // Make sure we are executing the subsystem
    let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
        .await
        .unwrap();
    assert!(recv_result.is_some());

    // Make sure the grandchild is still running
    sleep(Duration::from_millis(100)).await;
    assert!(matches!(
        drop_receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    drop(root_handle);

    // Make sure the subsystem got cancelled
    let recv_result = timeout(Duration::from_millis(100), drop_receiver.recv())
        .await
        .unwrap();
    assert!(recv_result.is_none());
}

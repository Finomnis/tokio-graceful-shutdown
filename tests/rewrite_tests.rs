use std::error::Error;

use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing_test::traced_test;

/// Error types
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
#[traced_test]
async fn normal_shutdown() {
    let subsystem = |s: SubsystemHandle| async move {
        s.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s: SubsystemHandle| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });
    let shutdown_token = toplevel._get_shutdown_token().clone();

    tokio::join!(
        async {
            sleep(Duration::from_millis(100)).await;
            shutdown_token.cancel();
        },
        async {
            let result = toplevel
                .handle_shutdown_requests(Duration::from_millis(400))
                .await;
            assert!(result.is_ok());
        },
    );
}

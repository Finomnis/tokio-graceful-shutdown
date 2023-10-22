use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemBuilder, SubsystemHandle, Toplevel};
use tracing_test::traced_test;

pub mod common;

use std::{
    error::Error,
    sync::{Arc, Mutex},
};

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
#[traced_test]
async fn leak_subsystem_handle() {
    let subsys_ext: Arc<Mutex<Option<SubsystemHandle>>> = Default::default();
    let subsys_ext2 = Arc::clone(&subsys_ext);

    let subsystem = move |subsys: SubsystemHandle| async move {
        subsys.on_shutdown_requested().await;

        *subsys_ext2.lock().unwrap() = Some(subsys);

        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(move |s| async move {
        s.start(SubsystemBuilder::new("subsys", subsystem));

        sleep(Duration::from_millis(100)).await;
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests(Duration::from_millis(100))
        .await;
    assert!(result.is_err());
    assert!(logs_contain(
        "The SubsystemHandle object must not be leaked out of the subsystem!"
    ));
}

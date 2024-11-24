//! This example shows to pass custom error types all the way through to the top,
//! to recover them from the return value of `handle_shutdown_requests`.

use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    errors::{GracefulShutdownError, SubsystemError},
    IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel,
};

#[derive(Debug, thiserror::Error)]
enum MyError {
    #[error("MyError.WithData: {0}")]
    WithData(u32),
    #[error("MyError.WithoutData")]
    WithoutData,
}

async fn subsys1(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem1 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem1 stopped.");

    Err(MyError::WithData(42))
}

async fn subsys2(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem2 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem2 stopped.");

    Err(MyError::WithoutData)
}

async fn subsys3(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem3 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem3 stopped.");

    panic!("This subsystem panicked.");
}

async fn subsys4(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem4 started.");
    sleep(Duration::from_millis(1000)).await;
    tracing::info!("Subsystem4 stopped.");

    // This subsystem would end normally but takes too long and therefore
    // will time out.
    Ok(())
}

async fn subsys5(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem5 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem5 stopped.");

    // This subsystem ended normally and should not show up in the list of
    // subsystem errors.
    Ok(())
}

// This subsystem implements the IntoSubsystem trait with a custom error type.
// The first generic is the error type returned from the `run()` function, the
// second generic is the error wrapper type used by Toplevel. In this case,
// both are identical.
struct Subsys6;

#[async_trait::async_trait]
impl IntoSubsystem<MyError, MyError> for Subsys6 {
    async fn run(self, _subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
        tracing::info!("Subsystem6 started.");
        sleep(Duration::from_millis(200)).await;
        tracing::info!("Subsystem6 stopped.");

        Err(MyError::WithData(69))
    }
}

#[tokio::main]
async fn main() -> Result<(), miette::Report> {
    // Init logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Setup and execute subsystem tree
    let errors = Toplevel::<MyError>::new(|s| async move {
        s.start(SubsystemBuilder::new("Subsys1", subsys1));
        s.start(SubsystemBuilder::new("Subsys2", subsys2));
        s.start(SubsystemBuilder::new("Subsys3", subsys3));
        s.start(SubsystemBuilder::new("Subsys4", subsys4));
        s.start(SubsystemBuilder::new("Subsys5", subsys5));
        s.start(SubsystemBuilder::new("Subsys6", Subsys6.into_subsystem()));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(500))
    .await;

    if let Err(e) = &errors {
        match e {
            GracefulShutdownError::SubsystemsFailed(_) => {
                tracing::warn!("Subsystems failed.")
            }
            GracefulShutdownError::ShutdownTimeout(_) => {
                tracing::warn!("Shutdown timed out.")
            }
        };

        for subsystem_error in e.get_subsystem_errors() {
            match subsystem_error {
                SubsystemError::Failed(name, e) => {
                    tracing::warn!("   Subsystem '{}' failed.", name);
                    match e.get_error() {
                        MyError::WithData(data) => {
                            tracing::warn!("      It failed with MyError::WithData({})", data)
                        }
                        MyError::WithoutData => {
                            tracing::warn!("      It failed with MyError::WithoutData")
                        }
                    }
                }
                SubsystemError::Panicked(name) => {
                    tracing::warn!("   Subsystem '{}' panicked.", name)
                }
            }
        }
    };

    Ok(errors?)
}

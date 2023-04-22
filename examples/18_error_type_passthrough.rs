//! This example shows to pass custom error types all the way through to the top,
//! to recover them from the return value of `handle_shutdown_requests`.

use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{
    errors::{GracefulShutdownError, SubsystemError},
    IntoSubsystem, SubsystemHandle, Toplevel,
};

#[derive(Debug, thiserror::Error)]
enum MyError {
    #[error("MyError.WithData: {0}")]
    WithData(u32),
    #[error("MyError.WithoutData")]
    WithoutData,
}

#[tracing::instrument(name = "Subsys1", skip_all)]
async fn subsys1(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem1 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem1 stopped.");

    Err(MyError::WithData(42))
}

#[tracing::instrument(name = "Subsys2", skip_all)]
async fn subsys2(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem2 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem2 stopped.");

    Err(MyError::WithoutData)
}

#[tracing::instrument(name = "Subsys3", skip_all)]
async fn subsys3(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem3 started.");
    sleep(Duration::from_millis(200)).await;
    tracing::info!("Subsystem3 stopped.");

    panic!("This subsystem panicked.");
}

#[tracing::instrument(name = "Subsys4", skip_all)]
async fn subsys4(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    tracing::info!("Subsystem4 started.");
    sleep(Duration::from_millis(1000)).await;
    tracing::info!("Subsystem4 stopped.");

    // This subsystem would end normally but takes too long and therefore
    // will time out.
    Ok(())
}

#[tracing::instrument(name = "Subsys5", skip_all)]
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
    #[tracing::instrument(name = "Subsys6", skip_all)]
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
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Create toplevel
    let errors = Toplevel::<MyError>::new()
        .start("Subsys1", subsys1)
        .start("Subsys2", subsys2)
        .start("Subsys3", subsys3)
        .start("Subsys4", subsys4)
        .start("Subsys5", subsys5)
        .start("Subsys6", Subsys6.into_subsystem())
        .catch_signals()
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;

    let mut sum = String::new();
    if let Err(e) = &errors {
        match e {
            GracefulShutdownError::SubsystemsFailed(_) => {
                sum.push_str(format!("Subsystems failed.\n").as_str());
            }
            GracefulShutdownError::ShutdownTimeout(_) => {
                sum.push_str(format!("Shutdown timed out.\n").as_str());
            }
        };

        for subsystem_error in e.get_subsystem_errors() {
            match subsystem_error {
                SubsystemError::Failed(name, e) => {
                    sum.push_str(format!("   Subsystem '{}' failed.\n", name).as_str());
                    match e.get_error() {
                        MyError::WithData(data) => {
                            sum.push_str(
                                format!("      It failed with MyError::WithData({})\n", data)
                                    .as_str(),
                            );
                        }
                        MyError::WithoutData => {
                            sum.push_str(
                                format!("      It failed with MyError::WithoutData\n").as_str(),
                            );
                        }
                    }
                }
                SubsystemError::Cancelled(name) => {
                    sum.push_str(format!("   Subsystem '{}' was cancelled.\n", name).as_str());
                }
                SubsystemError::Panicked(name) => {
                    sum.push_str(format!("   Subsystem '{}' panicked.\n", name).as_str());
                }
            }
        }
    };
    println!("{sum}");

    Ok(errors?)
}

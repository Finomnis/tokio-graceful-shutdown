//! This example shows to pass custom error types all the way through to the top,
//! to recover them from the return value of `handle_shutdown_requests`.

use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{GracefulShutdownError, IntoSubsystem, SubsystemHandle, Toplevel};

#[derive(Debug, thiserror::Error)]
enum MyError {
    #[error("MyError.WithData: {0}")]
    WithData(u32),
    #[error("MyError.WithoutData")]
    WithoutData,
}

async fn subsys1(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    log::info!("Subsystem1 started.");
    sleep(Duration::from_millis(200)).await;
    log::info!("Subsystem1 stopped.");

    Err(MyError::WithData(42))
}

async fn subsys2(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    log::info!("Subsystem2 started.");
    sleep(Duration::from_millis(200)).await;
    log::info!("Subsystem2 stopped.");

    Err(MyError::WithoutData)
}

async fn subsys3(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    log::info!("Subsystem3 started.");
    sleep(Duration::from_millis(200)).await;
    log::info!("Subsystem3 stopped.");

    panic!("This subsystem panicked.");
}

async fn subsys4(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    log::info!("Subsystem4 started.");
    sleep(Duration::from_millis(1000)).await;
    log::info!("Subsystem4 stopped.");

    // This subsystem would end normally but takes too long and therefore
    // will time out.
    Ok(())
}

async fn subsys5(_subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
    log::info!("Subsystem5 started.");
    sleep(Duration::from_millis(200)).await;
    log::info!("Subsystem5 stopped.");

    // This subsystem ended normally and should not show up in the list of
    // subsystem errors.
    Ok(())
}

struct Subsys6;

#[async_trait::async_trait]
impl IntoSubsystem<MyError, MyError> for Subsys6 {
    async fn run(self, _subsys: SubsystemHandle<MyError>) -> Result<(), MyError> {
        log::info!("Subsystem6 started.");
        sleep(Duration::from_millis(200)).await;
        log::info!("Subsystem6 stopped.");

        Err(MyError::WithData(69))
    }
}

#[tokio::main]
async fn main() -> Result<(), miette::Report> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    let errors = Toplevel::<MyError>::new()
        .start("Subsys1", subsys1)
        .start("Subsys2", subsys2)
        .start("Subsys3", subsys3)
        .start("Subsys4", subsys4)
        .start("Subsys5", subsys5)
        .start("Subsys6", Subsys6.into_subsystem())
        .catch_signals()
        .handle_shutdown_requests::<GracefulShutdownError<MyError>>(Duration::from_millis(500))
        .await;

    if let Err(e) = &errors {
        match e {
            GracefulShutdownError::SubsystemsFailed(_) => {
                log::warn!("Subsystems failed.")
            }
            GracefulShutdownError::ShutdownTimeout(_) => {
                log::warn!("Shutdown timed out.")
            }
        };

        for subsystem_error in e.get_subsystem_errors() {
            match subsystem_error {
                tokio_graceful_shutdown::SubsystemError::Failed(name, e) => {
                    log::warn!("   Subsystem '{}' failed.", name);
                    match e.get_error() {
                        MyError::WithData(data) => {
                            log::warn!("      It failed with MyError::WithData({})", data)
                        }
                        MyError::WithoutData => {
                            log::warn!("      It failed with MyError::WithoutData")
                        }
                    }
                }
                tokio_graceful_shutdown::SubsystemError::Cancelled(name) => {
                    log::warn!("   Subsystem '{}' was cancelled.", name)
                }
                tokio_graceful_shutdown::SubsystemError::Panicked(name) => {
                    log::warn!("   Subsystem '{}' panicked.", name)
                }
            }
        }
    };

    Ok(errors?)
}

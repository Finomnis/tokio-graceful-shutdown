use anyhow::{anyhow, Result};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::Toplevel;

mod common;
use common::slow_shutdown::SlowShutdownSubsystem;

#[tokio::test]
async fn normal_shutdown() -> Result<()> {
    let subsystem = SlowShutdownSubsystem::new(Duration::from_millis(500));

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    let runner = toplevel.wait_for_shutdown(Duration::from_millis(1000));

    let tester = async {
        sleep(Duration::from_millis(200)).await;
        shutdown_token.shutdown();
    };

    let (result, ()) = tokio::join!(runner, tester);
    result
}

#[tokio::test]
async fn shutdown_timeout() -> Result<()> {
    let subsystem = SlowShutdownSubsystem::new(Duration::from_millis(1000));

    let toplevel = Toplevel::new().start("subsys", subsystem);
    let shutdown_token = toplevel.get_shutdown_token().clone();

    let runner = toplevel.wait_for_shutdown(Duration::from_millis(500));

    let tester = async {
        sleep(Duration::from_millis(200)).await;
        shutdown_token.shutdown();
    };

    let (result, ()) = tokio::join!(runner, tester);

    match result {
        Ok(()) => Err(anyhow!("Should not be ok")),
        Err(_) => Ok(()),
    }
}

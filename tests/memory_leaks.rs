use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn subsys1(mut subsys: SubsystemHandle) -> Result<()> {
    subsys.start("Subsys2", subsys2);
    subsys.start("Subsys3", subsys3);
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem1 ...");
    sleep(Duration::from_millis(500)).await;
    panic!("Subsystem1 panicks!");
}

async fn subsys2(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem2 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Shutting down Subsystem2 ...");
    sleep(Duration::from_millis(400)).await;
    log::info!("Subsystem2 stopped.");
    Ok(())
}

async fn subsys3(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem3 started.");
    tokio::select! {
        _ = sleep(Duration::from_millis(200)) => {
            panic!("Sybsystem3 panics!");
        },
        _ = subsys.on_shutdown_requested() => (),
    };
    log::info!("Subsystem3 stopped.");
    Ok(())
}

#[tokio::test]
async fn test() {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    let xs = vec![0, 1, 2, 3];
    std::mem::forget(xs);

    // Create toplevel
    let result = Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await;

    assert!(result.is_err());
}

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(all(unix, not(test)))]
async fn wait_for_signal_impl() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => log::debug!("Received SIGTERM."),
        _ = signal_interrupt.recv() => log::debug!("Received SIGINT."),
    };
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(all(windows, not(test)))]
async fn wait_for_signal_impl() {
    use tokio::signal::ctrl_c;

    ctrl_c().await.unwrap();
    log::debug!("Received SIGINT.");
}

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
static TRIGGER: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub fn trigger_signal() {
    TRIGGER.store(true, Ordering::SeqCst);
}

#[cfg(test)]
async fn wait_for_signal_impl() {
    while !TRIGGER.load(Ordering::SeqCst) {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}

/// Registers Ctrl+C and SIGTERM handlers to cause a program shutdown.
/// Further, registers a custom panic handler to also initiate a shutdown.
/// Otherwise, a multi-threaded system would deadlock on panik.
pub async fn wait_for_signal() {
    wait_for_signal_impl().await
}

#[cfg(test)]
mod test {
    use tokio::time::{sleep, Duration};

    use crate::{SubsystemHandle, Toplevel};

    #[tokio::test]
    async fn shutdown_through_signal() {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

        let subsystem = |subsys: SubsystemHandle| async move {
            subsys.on_shutdown_requested().await;
            sleep(Duration::from_millis(200)).await;
            Ok(())
        };

        let toplevel = Toplevel::new().catch_signals();
        tokio::join!(
            async {
                sleep(Duration::from_millis(100)).await;
                super::trigger_signal();
            },
            async {
                let result = toplevel
                    .start("subsys", subsystem)
                    .handle_shutdown_requests(Duration::from_millis(400))
                    .await;
                assert!(result.is_ok());
            },
        );
    }
}

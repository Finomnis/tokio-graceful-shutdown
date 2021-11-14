use crate::initiate_shutdown;

use std::panic;

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => log::debug!("Received SIGTERM."),
        _ = signal_interrupt.recv() => log::debug!("Received SIGINT."),
    };

    initiate_shutdown();
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
async fn wait_for_signal() {
    use tokio::signal::ctrl_c;

    ctrl_c().await.unwrap();
    log::debug!("Received SIGINT.");

    initiate_shutdown();
}

/// Registers Ctrl+C and SIGTERM handlers to cause a program shutdown.
/// Further, registers a custom panic handler to also initiate a shutdown.
/// Otherwise, a multi-threaded system would deadlock on panik.
pub fn register_signal_handlers() {
    panic::set_hook(Box::new(|panic_info| {
        log::error!("ERROR: {}", panic_info);
        initiate_shutdown();
    }));

    tokio::spawn(wait_for_signal());
}

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
async fn wait_for_signal_impl() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => tracing::debug!("Received SIGTERM."),
        _ = signal_interrupt.recv() => tracing::debug!("Received SIGINT."),
    };
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
async fn wait_for_signal_impl() {
    use tokio::signal::ctrl_c;

    ctrl_c().await.unwrap();
    tracing::debug!("Received SIGINT.");
}

/// Registers Ctrl+C and SIGTERM handlers to cause a program shutdown.
/// Further, registers a custom panic handler to also initiate a shutdown.
/// Otherwise, a multi-threaded system would deadlock on panik.
pub(crate) async fn wait_for_signal() {
    wait_for_signal_impl().await
}

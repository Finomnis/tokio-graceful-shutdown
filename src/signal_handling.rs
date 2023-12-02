/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
async fn wait_for_signal_impl() {
    use tokio::signal::unix::{signal, SignalKind};

    // Infos here:
    // https://www.gnu.org/software/libc/manual/html_node/Termination-Signals.html
    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut signal_hangup = signal(SignalKind::hangup()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => tracing::debug!("Received SIGTERM."),
        _ = signal_interrupt.recv() => tracing::debug!("Received SIGINT."),
        _ = signal_hangup.recv() => tracing::debug!("Received SIGHUP."),
    };
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
async fn wait_for_signal_impl() {
    use tokio::signal::windows;

    // Infos here:
    // https://learn.microsoft.com/en-us/windows/console/handlerroutine
    let mut signal_c = windows::ctrl_c().unwrap();
    let mut signal_break = windows::ctrl_break().unwrap();
    let mut signal_close = windows::ctrl_close().unwrap();
    let mut signal_shutdown = windows::ctrl_shutdown().unwrap();

    tokio::select! {
        _ = signal_c.recv() => tracing::debug!("Received CTRL_C_EVENT."),
        _ = signal_break.recv() => tracing::debug!("Received CTRL_BREAK_EVENT."),
        _ = signal_close.recv() => tracing::debug!("Received CTRL_CLOSE_EVENT."),
        _ = signal_shutdown.recv() => tracing::debug!("Received CTRL_SHUTDOWN_EVENT."),
    };
}

/// Registers Ctrl+C and SIGTERM handlers to cause a program shutdown.
/// Further, registers a custom panic handler to also initiate a shutdown.
/// Otherwise, a multi-threaded system would deadlock on panik.
pub(crate) async fn wait_for_signal() {
    wait_for_signal_impl().await
}

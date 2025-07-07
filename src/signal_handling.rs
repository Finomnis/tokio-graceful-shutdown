use std::sync::Arc;

#[cfg(windows)]
use crate::ErrTypeTraits;
use crate::LogHandler;

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
async fn wait_for_signal_impl<E: ErrTypeTraits>(log: Arc<dyn LogHandler<E>>) {
    use tokio::signal::unix::{SignalKind, signal};

    // Infos here:
    // https://www.gnu.org/software/libc/manual/html_node/Termination-Signals.html
    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => log.signal_received("SIGTERM"),
        _ = signal_interrupt.recv() => log.signal_received("SIGINT"),
    };
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
async fn wait_for_signal_impl<E: ErrTypeTraits>(log: Arc<dyn LogHandler<E>>) {
    use tokio::signal::windows;

    // Infos here:
    // https://learn.microsoft.com/en-us/windows/console/handlerroutine
    let mut signal_c = windows::ctrl_c().unwrap();
    let mut signal_break = windows::ctrl_break().unwrap();
    let mut signal_close = windows::ctrl_close().unwrap();
    let mut signal_shutdown = windows::ctrl_shutdown().unwrap();

    tokio::select! {
        _ = signal_c.recv() => log.signal_received("CTRL_C"),
        _ = signal_break.recv() => log.signal_received("CTRL_BREAK"),
        _ = signal_close.recv() => log.signal_received("CTRL_CLOSE"),
        _ = signal_shutdown.recv() => log.signal_received("CTRL_SHUTDOWN"),
    };
}

/// Registers signal handlers and waits for a signal that
/// indicates a shutdown request.
pub(crate) async fn wait_for_signal<E: ErrTypeTraits>(log: Arc<dyn LogHandler<E>>) {
    wait_for_signal_impl(log).await
}

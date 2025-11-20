use std::io;

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
fn register_signals_impl() -> io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};

    // Infos here:
    // https://www.gnu.org/software/libc/manual/html_node/Termination-Signals.html
    let mut signal_terminate = signal(SignalKind::terminate())?;
    let mut signal_interrupt = signal(SignalKind::interrupt())?;

    Ok(async move {
        tokio::select! {
            _ = signal_terminate.recv() => tracing::debug!("Received SIGTERM."),
            _ = signal_interrupt.recv() => tracing::debug!("Received SIGINT."),
        }
    })
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
fn register_signals_impl() -> io::Result<impl Future<Output = ()>> {
    use tokio::signal::windows;

    // Infos here:
    // https://learn.microsoft.com/en-us/windows/console/handlerroutine
    let mut signal_c = windows::ctrl_c()?;
    let mut signal_break = windows::ctrl_break()?;
    let mut signal_close = windows::ctrl_close()?;
    let mut signal_shutdown = windows::ctrl_shutdown()?;

    Ok(async move {
        tokio::select! {
            _ = signal_c.recv() => tracing::debug!("Received CTRL_C."),
            _ = signal_break.recv() => tracing::debug!("Received CTRL_BREAK."),
            _ = signal_close.recv() => tracing::debug!("Received CTRL_CLOSE."),
            _ = signal_shutdown.recv() => tracing::debug!("Received CTRL_SHUTDOWN."),
        }
    })
}

/// Registers signal handlers and waits for a signal that
/// indicates a shutdown request.
pub(crate) fn register_signals() -> io::Result<impl Future<Output = ()>> {
    register_signals_impl()
}

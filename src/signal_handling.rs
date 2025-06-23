use async_trait::async_trait;

/// Waits for a signal that requests a graceful shutdown, like SIGTERM or SIGINT.
#[cfg(unix)]
async fn wait_for_signal_impl(mut hooks: impl SignalHooks) {
    use tokio::signal::unix::{SignalKind, signal};

    // Infos here:
    // https://www.gnu.org/software/libc/manual/html_node/Termination-Signals.html
    let mut signal_terminate = signal(SignalKind::terminate()).unwrap();
    let mut signal_interrupt = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = signal_terminate.recv() => hooks.on_sigterm().await,
        _ = signal_interrupt.recv() => hooks.on_sigint().await,
    };
}

/// Waits for a signal that requests a graceful shutdown, Ctrl-C (SIGINT).
#[cfg(windows)]
async fn wait_for_signal_impl(mut hooks: impl SignalHooks) {
    use tokio::signal::windows;

    // Infos here:
    // https://learn.microsoft.com/en-us/windows/console/handlerroutine
    let mut signal_c = windows::ctrl_c().unwrap();
    let mut signal_break = windows::ctrl_break().unwrap();
    let mut signal_close = windows::ctrl_close().unwrap();
    let mut signal_shutdown = windows::ctrl_shutdown().unwrap();

    tokio::select! {
        _ = signal_c.recv() => hooks.on_ctrl_c().await,
        _ = signal_break.recv() => hooks.on_ctrl_break().await,
        _ = signal_close.recv() => hooks.on_ctrl_close().await,
        _ = signal_shutdown.recv() => hooks.on_ctrl_shutdown().await,
    };
}

/// Registers signal handlers and waits for a signal that
/// indicates a shutdown request.
pub(crate) async fn wait_for_signal(hooks: impl SignalHooks) {
    wait_for_signal_impl(hooks).await
}

#[async_trait]
/// A trait that allows executing custom logic when specific OS signals are received.
///
/// Implementing this trait requires the `async_trait` dependency.
///
/// It can be passed to [`Toplevel::catch_signals_with_hooks`](crate::Toplevel::catch_signals_with_hooks).
/// All methods have a default implementation that logs the received signal at the `DEBUG` level,
/// so you only need to implement the ones you are interested in.
pub trait SignalHooks: Send + 'static {
    /// Called when a `SIGTERM` signal is received (on Unix).
    #[cfg(unix)]
    async fn on_sigterm(&mut self) {
        tracing::debug!("Received SIGTERM.")
    }

    /// Called when a `SIGINT` signal is received (on Unix), usually from Ctrl+C.
    #[cfg(unix)]
    async fn on_sigint(&mut self) {
        tracing::debug!("Received SIGINT.")
    }

    /// Called when a `CTRL_C` signal is received (on Windows).
    #[cfg(windows)]
    async fn on_ctrl_c(&mut self) {
        tracing::debug!("Received CTRL_C.")
    }

    /// Called when a `CTRL_BREAK` signal is received (on Windows).
    #[cfg(windows)]
    async fn on_ctrl_break(&mut self) {
        tracing::debug!("Received CTRL_BREAK.")
    }

    /// Called when a `CTRL_CLOSE` signal is received (on Windows), e.g., when the console window is
    /// closed.
    #[cfg(windows)]
    async fn on_ctrl_close(&mut self) {
        tracing::debug!("Received CTRL_CLOSE.")
    }

    /// Called when a `CTRL_SHUTDOWN` signal is received (on Windows), e.g., when the system is
    /// shutting down.
    #[cfg(windows)]
    async fn on_ctrl_shutdown(&mut self) {
        tracing::debug!("Received CTRL_SHUTDOWN.")
    }
}

/// A default implementation of [`SignalHooks`] that provides logging for received signals.
///
/// This is used by [`Toplevel::catch_signals`](crate::Toplevel::catch_signals).
pub struct DefaultSignalHooks;

impl SignalHooks for DefaultSignalHooks {}

#![allow(unused_imports)]

mod event;

use async_trait::async_trait;
pub use event::Event;
use nix::sys::signal;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::error::Error;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::time::sleep;
use tokio_graceful_shutdown::{SignalHooks, SubsystemBuilder, SubsystemHandle, Toplevel};

/// Wrapper type to simplify lambdas
pub type BoxedError = Box<dyn Error + Sync + Send>;
pub type BoxedResult = Result<(), BoxedError>;

struct MockSignalHooks {
    signal: Arc<OnceLock<Signal>>,
    capture_only: Signal,
}

impl MockSignalHooks {
    fn new(capture_only: Signal) -> Self {
        Self {
            signal: Arc::new(OnceLock::new()),
            capture_only,
        }
    }
}

#[async_trait]
impl SignalHooks for MockSignalHooks {
    async fn on_sigterm(&mut self) {
        if matches!(self.capture_only, Signal::SIGTERM) {
            self.signal.set(Signal::SIGTERM).unwrap()
        }
    }
    async fn on_sigint(&mut self) {
        if matches!(self.capture_only, Signal::SIGINT) {
            self.signal.set(Signal::SIGINT).unwrap()
        }
    }
}

async fn run_toplevel_with_signal_hooks(hooks: impl SignalHooks) {
    let subsystem = async move |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });

    let result = toplevel
        .catch_signals_with_hooks(hooks)
        .handle_shutdown_requests(Duration::from_millis(500))
        .await;

    assert!(result.is_ok());
}

pub async fn test_signal_hook(signal_to_test: Signal) {
    let hooks = MockSignalHooks::new(signal_to_test);
    let signal = hooks.signal.clone();

    tokio::spawn(async move {
        sleep(Duration::from_millis(100)).await;
        signal::kill(Pid::this(), signal_to_test).unwrap();
    });

    run_toplevel_with_signal_hooks(hooks).await;

    assert_eq!(
        signal.get(),
        Some(&signal_to_test),
        "{signal_to_test} hook was not called: got {signal:?}"
    );
}

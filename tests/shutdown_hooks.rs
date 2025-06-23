mod common;

use anyhow::anyhow;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, sleep};
use tracing_test::traced_test;

use tokio_graceful_shutdown::{
    ErrTypeTraits, ShutdownHooks, SubsystemBuilder, SubsystemHandle, Toplevel,
    errors::{GracefulShutdownError, SubsystemError},
};

use common::{BoxedError, BoxedResult};

#[derive(Clone, Debug, PartialEq)]
enum HookEvent {
    SubsystemsFinished,
    ShutdownRequested,
    ShutdownFinished(Vec<String>),
    ShutdownTimeout,
}

#[derive(Clone)]
struct MockShutdownHooks {
    events: Arc<Mutex<Vec<HookEvent>>>,
}

impl MockShutdownHooks {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn events(&self) -> Vec<HookEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl ShutdownHooks for MockShutdownHooks {
    async fn on_subsystems_finished(&mut self) {
        self.events
            .lock()
            .unwrap()
            .push(HookEvent::SubsystemsFinished);
    }

    async fn on_shutdown_requested(&mut self) {
        self.events
            .lock()
            .unwrap()
            .push(HookEvent::ShutdownRequested);
    }

    async fn on_shutdown_finished<ErrType: ErrTypeTraits>(
        &mut self,
        errors: &[SubsystemError<ErrType>],
    ) {
        let mut error_summary: Vec<String> = errors.iter().map(|e| e.name().to_string()).collect();

        // sort for deterministic test results
        error_summary.sort();

        self.events
            .lock()
            .unwrap()
            .push(HookEvent::ShutdownFinished(error_summary));
    }

    async fn on_shutdown_timeout(&mut self) {
        self.events.lock().unwrap().push(HookEvent::ShutdownTimeout);
    }
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_on_subsystems_finished_hook() {
    let hooks = MockShutdownHooks::new();

    let subsystem = |_subsys: SubsystemHandle| async {
        sleep(Duration::from_millis(50)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));
    });

    let result = toplevel
        .handle_shutdown_requests_with_hooks(Duration::from_millis(200), hooks.clone())
        .await;

    assert!(result.is_ok());

    let events = hooks.events();
    assert_eq!(events, vec![HookEvent::SubsystemsFinished]);
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_on_shutdown_requested_and_finished_hooks() {
    let hooks = MockShutdownHooks::new();

    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(50)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests_with_hooks(Duration::from_millis(200), hooks.clone())
        .await;

    assert!(result.is_ok());

    let events = hooks.events();
    assert_eq!(
        events,
        vec![
            HookEvent::ShutdownRequested,
            HookEvent::ShutdownFinished(vec![])
        ]
    );
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_on_shutdown_timeout_hook() {
    let hooks = MockShutdownHooks::new();

    let subsystem = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(200)).await; // This will timeout
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("subsys", subsystem));
        s.request_shutdown();
    });

    let result = toplevel
        .handle_shutdown_requests_with_hooks(Duration::from_millis(100), hooks.clone())
        .await;

    assert!(matches!(
        result,
        Err(GracefulShutdownError::ShutdownTimeout(_))
    ));

    let events = hooks.events();
    assert_eq!(
        events,
        vec![HookEvent::ShutdownRequested, HookEvent::ShutdownTimeout]
    );
}

#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_on_shutdown_finished_with_errors_hook() {
    let hooks = MockShutdownHooks::new();

    let subsys_fail = async |_subsys: SubsystemHandle| {
        sleep(Duration::from_millis(20)).await;
        Err(anyhow!("I failed"))
    };
    let subsys_panic = async |_subsys: SubsystemHandle<BoxedError>| {
        sleep(Duration::from_millis(20)).await;
        panic!("I panicked")
    };
    let subsys_ok = async |subsys: SubsystemHandle| {
        subsys.on_shutdown_requested().await;
        sleep(Duration::from_millis(50)).await;
        BoxedResult::Ok(())
    };

    let toplevel = Toplevel::new(async move |s| {
        s.start(SubsystemBuilder::new("fail", subsys_fail));
        s.start::<anyhow::Error, _, _>(SubsystemBuilder::new("panic", subsys_panic));
        s.start(SubsystemBuilder::new("ok", subsys_ok));
    });

    let result = toplevel
        .handle_shutdown_requests_with_hooks(Duration::from_millis(200), hooks.clone())
        .await;

    assert!(matches!(
        result,
        Err(GracefulShutdownError::SubsystemsFailed(_))
    ));

    let events = hooks.events();
    assert_eq!(
        events,
        vec![
            HookEvent::ShutdownRequested,
            HookEvent::ShutdownFinished(vec!["/fail".to_string(), "/panic".to_string()])
        ]
    );
}

/*
- knows children
- runs subsystems
- CAN REACT TO subsystem stop/error/panic
- can decide whether subsystem errors bubble up
- has an on_finish function to give the parent the chance to react to its shutdown

- can do global, scoped and local shutdown

- question: who should actually run the subsystem?

- question: where will errors be propagated?
    - into callback
    - then, collect them in class (for nested) or forward them to handle (for detached)
*/

use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

use futures::Future;
use tokio::task::JoinError;

use miette::{Diagnostic, Result};

use crate::{
    internal::{
        subsystem_tree::parent::SubsystemTreeParent,
        utils::{
            event::{Event, EventTrigger},
            shutdown_token::ShutdownToken,
        },
    },
    BoxedError,
};

#[derive(thiserror::Error)]
#[error(transparent)]
pub struct BoxedJoinError(JoinError);
impl fmt::Debug for BoxedJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
impl Diagnostic for BoxedJoinError {}

#[derive(thiserror::Error, Debug, Diagnostic)]
pub enum SubsystemError {
    #[error("Error in subsystem {0}")]
    #[diagnostic(code(tokio_graceful_shutdown::subsystem::failed))]
    Failed(String, #[source] Box<dyn Error + Send + Sync>),
    #[error("Subsystem {0} was aborted")]
    #[diagnostic(code(tokio_graceful_shutdown::subsystem::aborted))]
    Cancelled(String),
    #[error("Subsystem {0} panicked")]
    #[diagnostic(code(tokio_graceful_shutdown::subsystem::panicked))]
    Panicked(String),
}

pub struct SubsystemTreeNode {
    name: String,
    parent: Box<dyn SubsystemTreeParent>,
    children: Mutex<Vec<Arc<SubsystemTreeNode>>>,
    finished: Event,
    set_finished: EventTrigger,
    abort_requested: Event,
    set_abort_requested: EventTrigger,
    shutdown_token_local: ShutdownToken,
    shutdown_token_group: ShutdownToken,
    shutdown_token_global: ShutdownToken,
    errors: Mutex<Vec<(String, SubsystemError)>>,
}

impl SubsystemTreeNode {
    pub fn new(
        name: &str,
        parent: Box<dyn SubsystemTreeParent>,
        shutdown_token_global: ShutdownToken,
        shutdown_token_group: ShutdownToken,
        shutdown_token_local: ShutdownToken,
    ) -> Self {
        let (abort_requested, set_abort_requested) = Event::create();
        let (finished, set_finished) = Event::create();

        Self {
            name: name.to_string(),
            parent,
            abort_requested,
            set_abort_requested,
            finished,
            set_finished,
            shutdown_token_global,
            shutdown_token_group,
            shutdown_token_local,
            children: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
        }
    }

    //fn create_child(&self, detached: bool) -> ChildHandle {}

    pub async fn execute<Fut: 'static + Future<Output = Result<(), BoxedError>> + Send>(
        &self,
        subsystem_future: Fut,
    ) -> Result<(), SubsystemError> {
        let mut joinhandle = tokio::spawn(subsystem_future);
        let joinhandle_ref = &mut joinhandle;

        fn handle_subsystem_outcome(
            obj: &SubsystemTreeNode,
            child: Result<Result<(), Box<dyn Error + Sync + Send>>, JoinError>,
        ) -> Result<(), SubsystemError> {
            match child {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(SubsystemError::Failed(obj.name.to_string(), e)),
                Err(e) => Err(if e.is_cancelled() {
                    SubsystemError::Cancelled(obj.name.to_string())
                } else {
                    SubsystemError::Panicked(obj.name.to_string())
                }),
            }
        }

        tokio::select! {
            result = joinhandle_ref => {
                handle_subsystem_outcome(self, result)
            },
            _ = self.abort_requested.wait() => {
                joinhandle.abort();
                handle_subsystem_outcome(self, joinhandle.await)
            }
        }
    }

    //fn abort(&self) ->
}

#[cfg(test)]
mod tests {
    use crate::internal::{
        subsystem_tree::parent::DummyParent, utils::shutdown_token::create_shutdown_token,
    };

    use super::*;

    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn dummy() {
        async fn dummy_subsys() -> Result<(), Box<dyn Error + Send + Sync>> {
            println!("Dummy subsys");
            panic!("AAAAA");
        }

        let parent = Box::new(DummyParent {});

        let shutdown_token = create_shutdown_token();

        let node = SubsystemTreeNode::new(
            "aaa",
            parent,
            shutdown_token.clone(),
            shutdown_token.clone(),
            shutdown_token,
        );

        let result = node.execute(dummy_subsys()).await;

        match result {
            Ok(()) => println!("Ok."),
            Err(e) => println!("Error: {:?}", e),
        }

        //assert!(false);
    }
}

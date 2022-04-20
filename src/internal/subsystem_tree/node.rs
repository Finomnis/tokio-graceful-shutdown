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
    sync::{Arc, Mutex},
};

use futures::Future;
use tokio::task::JoinError;

use miette::Result;

use crate::{
    errors::SubsystemError,
    internal::{
        subsystem_tree::parent::SubsystemTreeParent,
        utils::{
            event::{Event, EventTrigger},
            shutdown_token::ShutdownToken,
        },
    },
    BoxedError,
};

pub struct SubsystemTreeNode {
    name: String,
    parent: Box<dyn SubsystemTreeParent>,
    children: Mutex<Vec<Arc<SubsystemTreeNode>>>,
    child_errors: Mutex<Vec<(String, SubsystemError)>>,
    /// Indicates that the subsystem and all its children are finished
    finished: Event,
    set_finished: EventTrigger,
    abort_requested: Event,
    set_abort_requested: EventTrigger,
    shutdown_token_local: ShutdownToken,
    shutdown_token_group: ShutdownToken,
    shutdown_token_global: ShutdownToken,
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
            child_errors: Mutex::new(Vec::new()),
        }
    }

    //fn create_child(&self, detached: bool) -> ChildHandle {}

    /// Executes the subsystem future.
    ///
    /// This function will block and must therefore most likely be wrapped in a tokio::spawn.
    pub async fn execute<Fut: 'static + Future<Output = Result<(), BoxedError>> + Send>(
        &self,
        subsystem_future: Fut,
    ) -> Result<(), SubsystemError> {
        // Run tokio::spawn internally again. This one is to catch and process panics.
        let mut joinhandle = tokio::spawn(subsystem_future);
        let joinhandle_ref = &mut joinhandle;

        /// Maps the complicated return value of the subsystem joinhandle to an appropriate error
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

    pub fn abort(&self) {
        self.set_abort_requested.set();
    }
}

#[cfg(test)]
mod tests {
    use crate::internal::{
        subsystem_tree::parent::DummyParent, utils::shutdown_token::create_shutdown_token,
    };

    use super::*;

    mod returnvalues {
        use super::*;

        fn create_node() -> SubsystemTreeNode {
            let parent = Box::new(DummyParent {});
            let shutdown_token = create_shutdown_token();

            SubsystemTreeNode::new(
                "MyGreatSubsystem",
                parent,
                shutdown_token.clone(),
                shutdown_token.clone(),
                shutdown_token,
            )
        }

        #[tokio::test]
        async fn ok() {
            async fn subsys() -> Result<(), Box<dyn Error + Send + Sync>> {
                Ok(())
            }

            let node = create_node();
            let result = node.execute(subsys()).await;

            assert!(matches!(result, Ok(())));
        }

        #[tokio::test]
        async fn error() {
            async fn subsys() -> Result<(), Box<dyn Error + Send + Sync>> {
                Err("ErrorText".into())
            }

            let node = create_node();
            let result = node.execute(subsys()).await;

            if let Err(SubsystemError::Failed(name, e)) = result {
                assert_eq!(name, "MyGreatSubsystem");
                assert_eq!(format!("{}", e), "ErrorText");
            } else {
                assert!(false, "Result is incorrect.");
            }
        }

        #[tokio::test]
        async fn panic() {
            async fn subsys() -> Result<(), Box<dyn Error + Send + Sync>> {
                panic!();
            }

            let node = create_node();
            let result = node.execute(subsys()).await;

            if let Err(SubsystemError::Panicked(name)) = result {
                assert_eq!(name, "MyGreatSubsystem");
            } else {
                assert!(false, "Result is incorrect.");
            }
        }

        #[tokio::test]
        async fn cancelled() {
            let node = create_node();
            node.abort();

            let result = node.execute(std::future::pending()).await;

            if let Err(SubsystemError::Cancelled(name)) = result {
                assert_eq!(name, "MyGreatSubsystem");
            } else {
                assert!(false, "Result is incorrect.");
            }
        }
    }
}

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

- prevent child spawning when subsystem is finished


- use atomic counter for consistency enforcement
    - only allow spawning if counter is != 0
    - every subsystem counts as 1
    - the lambda itself also counts as 1
    - class should allow awaiting for 0
    - reduce 1 every time a subsystem stops
    - reduce by 1 when lambda finishes
*/

use std::{
    collections::HashSet,
    error::Error,
    hash::Hash,
    sync::{Arc, Mutex},
};

use futures::Future;
use tokio::sync::oneshot;
use tokio::task::JoinError;

use miette::Result;

use crate::{
    api::subsystem_handle::SubsystemHandle,
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

use super::parent::DummyParent;

pub struct ChildHandle {
    child: Arc<SubsystemTreeNode>,
    result: Option<oneshot::Receiver<Result<(), SubsystemError>>>,
}

pub struct SubsystemTreeNode {
    name: String,
    parent: Box<dyn SubsystemTreeParent + Send + Sync>,
    children: Mutex<HashSet<Arc<SubsystemTreeNode>>>,
    child_errors: Mutex<Vec<SubsystemError>>,
    /// Indicates that the subsystem and all its children are finished
    finished: Event,
    set_finished: EventTrigger,
    abort_requested: Event,
    set_abort_requested: EventTrigger,
    shutdown_token_local: ShutdownToken,
    shutdown_token_group: ShutdownToken,
    shutdown_token_global: ShutdownToken,
}

impl Hash for SubsystemTreeNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(&self, state);
    }
}

impl Eq for SubsystemTreeNode {}

impl PartialEq for SubsystemTreeNode {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(&self, &other)
    }
}

impl SubsystemTreeNode {
    pub fn new(
        name: &str,
        parent: Box<dyn SubsystemTreeParent + Send + Sync>,
        shutdown_token_global: ShutdownToken,
        shutdown_token_group: ShutdownToken,
        shutdown_token_local: ShutdownToken,
    ) -> Arc<Self> {
        let (abort_requested, set_abort_requested) = Event::create();
        let (finished, set_finished) = Event::create();

        let node = Arc::new(Self {
            name: name.to_string(),
            parent,
            abort_requested,
            set_abort_requested,
            finished,
            set_finished,
            shutdown_token_global,
            shutdown_token_group,
            shutdown_token_local,
            children: Mutex::new(HashSet::new()),
            child_errors: Mutex::new(Vec::new()),
        });

        node
    }

    /// Spawns a child node.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the child node
    /// * `child_lambda` - The child subsystem function
    /// * `self_reference` - A `Arc` reference to self, that will be used to keep the
    ///                      current node alive while the child exists
    /// * `detached` - Whether or not child node will be a detached node.
    ///                Detached nodes don't propagate errors upwards, but instead
    ///                only shut down a local subtree on error or panic.
    ///                Errors will have to be handled by the caller of this function.
    pub fn spawn_child<
        Err: Into<BoxedError>,
        Fut: 'static + Future<Output = Result<(), Err>> + Send,
        S: 'static + FnOnce(SubsystemHandle) -> Fut + Send,
    >(
        &self,
        name: &str,
        child_lambda: S,
        self_reference: Arc<Self>,
        detached: bool,
    ) -> ChildHandle {
        // Create shutdown tokens for the child.
        // If child is detached, start a new shutdown group.
        let shutdown_token_child = self.shutdown_token_local.child_token();
        let shutdown_token_child_group = if detached {
            shutdown_token_child.clone()
        } else {
            self.shutdown_token_group.clone()
        };

        // TODO: Synchronize child spawning with set_finished, so that a finished subsystem can never be unfinished again.
        // TODO: replace dummy parent with weak pointer to actual parent
        let node = SubsystemTreeNode::new(
            name,
            Box::new(DummyParent {}),
            self.shutdown_token_global.clone(),
            shutdown_token_child_group,
            shutdown_token_child,
        );

        // Create SubsystemHandle
        let subsys_handle = SubsystemHandle {};

        // Store child in array of children
        self.children.lock().unwrap().insert(node.clone());

        // Set up connection to transfer subsystem result
        let (result_sender, result_receiver) = if detached {
            let (sender, receiver) = oneshot::channel();
            (Some(sender), Some(receiver))
        } else {
            (None, None)
        };

        // Create child handle for further processing of the spawned child
        let child_handle = ChildHandle {
            child: node.clone(),
            result: result_receiver,
        };

        // Handle child process return values
        // For that, we need a strong pointer to the current node.
        // Create it from the weak pointer we have stored.
        // It is trivially provable that the weak pointer is valid, because
        // we are inside of a member function here.
        tokio::spawn(async move {
            // Spawn child process
            let child_future =
                node.execute(async { child_lambda(subsys_handle).await.map_err(|e| e.into()) });

            let result = child_future.await;

            // Attempt to send the result to the oneshot pipe
            let result = if let Some(sender) = result_sender {
                match sender.send(result) {
                    Err(e) => e,
                    Ok(()) => Ok(()),
                }
            } else {
                result
            };

            // If it failed, store the error in the local error list
            if let Err(e) = result {
                self_reference.child_errors.lock().unwrap().push(e);
            }

            //self_reference.children.lock().unwrap().remove(&node);

            // TODO: check if the subsystem is now finished and disable spawning of new subsystems
        });

        child_handle
    }

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

    fn create_node() -> Arc<SubsystemTreeNode> {
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

    mod error_propagation {
        use std::time::Duration;

        use super::*;

        #[tokio::test]
        async fn detached() {
            async fn subsys(_subsys: SubsystemHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
                Err("TragicError".into())
            }

            let node = create_node();
            let child_handle = node.spawn_child("ChildSubsys".into(), subsys, node.clone(), true);

            let result = child_handle.result.unwrap().await.unwrap();

            if let Err(SubsystemError::Failed(name, e)) = result {
                assert_eq!(name, "ChildSubsys");
                assert_eq!(format!("{}", e), "TragicError");
            } else {
                assert!(false, "Result is incorrect.");
            }
        }

        #[tokio::test]
        async fn nested() {
            async fn subsys(_subsys: SubsystemHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
                Err("TragicError".into())
            }

            let node = create_node();
            let child_handle = node.spawn_child("ChildSubsys".into(), subsys, node.clone(), false);

            assert!(child_handle.result.is_none());

            tokio::time::sleep(Duration::from_millis(100)).await;

            let child_errors = node.child_errors.lock().unwrap();

            assert_eq!(child_errors.len(), 1);
            let result = child_errors.get(0).unwrap();

            if let SubsystemError::Failed(name, e) = result {
                assert_eq!(name, "ChildSubsys");
                assert_eq!(format!("{}", e), "TragicError");
            } else {
                assert!(false, "Result is incorrect.");
            }
        }
    }

    mod return_values {
        use super::*;

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

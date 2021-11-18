use std::rc::Rc;
use std::{collections::HashMap, pin::Pin};

use anyhow::Result;
use async_trait::async_trait;
use tokio::task::JoinHandle;

use crate::shutdown_token::ShutdownToken;

pub struct SubsystemData {
    name: &'static str,
    subsystems: Vec<Rc<SubsystemData>>,
    shutdown_token: ShutdownToken,
    join_handle: JoinHandle<()>,
}

pub struct SubsystemHandle {
    shutdown_token: ShutdownToken,
    data: Rc<SubsystemData>,
}

impl SubsystemData {
    pub fn new(
        name: &'static str,
        shutdown_token: ShutdownToken,
        join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            name,
            subsystems: Vec::new(),
            shutdown_token,
            join_handle,
        }
    }
}

impl SubsystemHandle {
    pub fn new(data: Rc<SubsystemData>) -> Self {
        Self {
            shutdown_token: data.shutdown_token.clone(),
            data,
        }
    }
}

// impl SubsystemHandle {
//     pub fn new(subsystem: Box<dyn AsyncSubsystem>, shutdown_token: ShutdownToken) -> Self {
//         Self {
//             subsystem,
//             shutdown_token,
//         }
//     }

//     pub fn start<S: AsyncSubsystem + 'static + Send>(
//         &mut self,
//         name: &'static str,
//         subsystem: S,
//     ) -> &mut Self {
//         let boxed_subsys = Box::new(subsystem);
//         let shutdown_token = self.shutdown_token.clone();

//         tokio::spawn(async move {
//             let handle = Box::new(SubsystemHandle::new(boxed_subsys, shutdown_token));
//             handle.run();
//         });

//         self
//     }

//     pub async fn on_shutdown_request(&self) {
//         self.shutdown_token.wait_for_shutdown().await
//     }

//     pub fn initiate_shutdown(&self) {
//         self.shutdown_token.shutdown();
//     }

//     pub fn shutdown_token(&self) -> ShutdownToken {
//         self.shutdown_token.clone()
//     }

//     fn run(mut self) {}
// }

#[async_trait(?Send)]
pub trait AsyncSubsystem {
    async fn run(&mut self, inst: SubsystemHandle) -> Result<()>;
}

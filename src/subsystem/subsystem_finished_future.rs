use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::utils::JoinerTokenRef;

use super::SubsystemFinishedFuture;

impl SubsystemFinishedFuture {
    pub(crate) fn new(joiner: JoinerTokenRef) -> Self {
        Self {
            future: Box::pin(async move { joiner.join().await }),
        }
    }
}

impl Future for SubsystemFinishedFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(cx)
    }
}

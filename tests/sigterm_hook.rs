//! Tests for the custom signal hooks functionality. These tests are Unix-only because simulating
//! signals on Windows is not straightforward.
//!
//! This is separate from `sigint_hook.rs` because otherwise `cargo` would run these tests on one
//! process, which would result in conflicts when trying to send signals to the same process.
#![cfg(unix)]

use nix::sys::signal::Signal;
use tracing_test::traced_test;

mod common;

#[tokio::test(start_paused = true)]
#[traced_test]
async fn test_sigterm_hook() {
    common::test_signal_hook(Signal::SIGTERM).await
}
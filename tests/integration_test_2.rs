// Required for test coverage
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use anyhow::anyhow;
use tokio::time::{sleep, timeout, Duration};
use tokio_graceful_shutdown::{
    errors::{GracefulShutdownError, SubsystemError, SubsystemJoinError},
    ErrorAction, IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel,
};
use tracing_test::traced_test;

pub mod common;
use common::Event;

use std::error::Error;

/// Wrapper function to simplify lambdas
type BoxedError = Box<dyn Error + Sync + Send>;
type BoxedResult = Result<(), BoxedError>;

#[tokio::test]
#[traced_test]
async fn dummy() {}

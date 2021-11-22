//! This example is not an actual example.
//!
//! It is the counterpart to leak_test

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let xs = vec![0, 1, 2, 3];
    std::mem::forget(xs);

    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    sleep(Duration::from_millis(100)).await;

    log::info!("Hello world!");
    Ok(())
}

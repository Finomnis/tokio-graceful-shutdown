//! This example is not an actual example.
//!
//! It is just a demonstrator to show that this crate does not leak memory.
//! It gets used by the CI to perform a very crude leak check.
//!
//! Run this example with the environment variable:
//!     sudo apt install valgrind
//!     cargo build --example leak_test
//!     valgrind --leak-check=yes target/debug/examples/leak_test
//!
//! This will print allocation information, including the amount of leaked memory.

use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    sleep(Duration::from_millis(100)).await;

    log::info!("Hello world!");
    Ok(())
}

# tokio-graceful-shutdown

[![Crates.io](https://img.shields.io/crates/v/tokio-graceful-shutdown)](https://crates.io/crates/tokio-graceful-shutdown)
[![Crates.io](https://img.shields.io/crates/d/tokio-graceful-shutdown)](https://crates.io/crates/tokio-graceful-shutdown)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](https://github.com/Finomnis/tokio-graceful-shutdown/blob/main/LICENSE)
[![Build Status](https://img.shields.io/github/workflow/status/Finomnis/tokio-graceful-shutdown/CI/main)](https://github.com/Finomnis/tokio-graceful-shutdown/actions/workflows/ci.yml?query=branch%3Amain)
[![docs.rs](https://img.shields.io/docsrs/tokio-graceful-shutdown)](https://docs.rs/tokio-graceful-shutdown)


This crate provides utility functions to perform a graceful shutdown on tokio-rs based services.

Specifically, it provides:

- Listening for shutdown requests from within subsystems
- Manual shutdown initiation from within subsystems
- Automatic shutdown on
    - SIGINT/SIGTERM/Ctrl+C
    - Subsystem failure
    - Subsystem panic
- Clean shutdown procedure with timeout and error propagation
- Subsystem nesting

## Usage Example

```rust
async fn subsys1(subsys: SubsystemHandle) -> Result<()>
{
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}
```

This shows a very basic asynchronous subsystem that simply starts, waits for the system shutdown to be triggered, and then stops itself.

This subsystem can now be executed like this:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
```

The `Toplevel` object is the root object of the subsystem tree.
Subsystems can then be started using the `start()` functionality of the toplevel object.

The `catch_signals()` method signals the `Toplevel` object to listen for SIGINT/SIGTERM/Ctrl+C and initiate a shutdown thereafter.

`wait_for_shutdown()` is the final and most important method of `Toplevel`. It idles until the system enters the shutdown mode. Then, it collects all the return values of the subsystems and determines the global error state, and makes sure shutdown completes within the given timeout.
Lastly, it returns an error value that can be directly used as a return code for `main()`.

Further examples can be seen in the **examples** folder.

## Building

To use this library in your project, add the following to the `[dependencies]` section of `Cargo.toml`:
```toml
[dependencies]
tokio-graceful-shutdown = "0.3"
```

To run one of the examples (here `01_normal_shutdown.rs`), simply enter the repository folder and execute:
```bash
cargo run --example 01_normal_shutdown
```


## Motivation

Performing a graceful shutdown on an asynchronous system is a non-trivial problem. There are several solutions, but they all have their drawbacks:

- Global cancellation by forking with `tokio::select`. This is a wide-spread solution, but has the drawback that the cancelled tasks cannot react to it, so it's impossible for them to shut down gracefully.
- Forking with `tokio::spawn` and signalling the desire to shutdown running tasks with mechanisms like `tokio::CancellationToken`. This allows tasks to shut down gracefully, but requires a lot of boilerplate code, like
  - Passing the tokens to the tasks
  - Waiting for the tasks to finish
  - Implementing a timeout mechanism to prevent hangs
  - Collecting subsystem return values
  - Making sure that subsystem errors get handled correctly

  If then further functionality is required, as listening for signals like SIGINT or SIGTERM, the boilerplate code becomes quite messy.

And this is exactly what this crate aims to provide: clean abstractions to all this boilerplate code.


## Contributions

Contributions are welcome!

I primarily wrote this crate for my own convenience, so any ideas for improvements are
greatly appreciated.

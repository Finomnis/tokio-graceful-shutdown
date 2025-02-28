# tokio-graceful-shutdown

[![Crates.io](https://img.shields.io/crates/v/tokio-graceful-shutdown)](https://crates.io/crates/tokio-graceful-shutdown)
[![Crates.io](https://img.shields.io/crates/d/tokio-graceful-shutdown)](https://crates.io/crates/tokio-graceful-shutdown)
[![License](https://img.shields.io/crates/l/tokio-graceful-shutdown)](https://github.com/Finomnis/tokio-graceful-shutdown/blob/main/LICENSE-MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/Finomnis/tokio-graceful-shutdown/ci.yml?branch=main)](https://github.com/Finomnis/tokio-graceful-shutdown/actions/workflows/ci.yml?query=branch%3Amain)
[![docs.rs](https://img.shields.io/docsrs/tokio-graceful-shutdown)](https://docs.rs/tokio-graceful-shutdown)
[![Coverage Status](https://img.shields.io/codecov/c/github/Finomnis/tokio-graceful-shutdown)](https://app.codecov.io/github/Finomnis/tokio-graceful-shutdown)

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
- Partial shutdown of a selected subsystem tree

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

This shows a very basic asynchronous subsystem that simply starts, waits for the program shutdown to be triggered, and then stops itself.

This subsystem can now be executed like this:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    Toplevel::new(async |s| {
        s.start(SubsystemBuilder::new("Subsys1", subsys1))
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_millis(1000))
    .await
    .map_err(Into::into)
}
```

The `Toplevel` object is the root object of the subsystem tree.
Subsystems can then be started in it using the `start()` method
of its `SubsystemHandle` object.

The `catch_signals()` method signals the `Toplevel` object to listen for SIGINT/SIGTERM/Ctrl+C and initiate a shutdown thereafter.

`handle_shutdown_requests()` is the final and most important method of `Toplevel`. It idles until the program enters the shutdown mode. Then, it collects all the return values of the subsystems, determines the global error state and makes sure the shutdown completes within the given timeout.
Lastly, it returns an error value that can be directly used as a return code for `main()`.

Further examples can be seen in the [**examples**](https://github.com/Finomnis/tokio-graceful-shutdown/tree/main/examples) folder.

## Building

To use this library in your project, enter your project directory and run:
```bash
cargo add tokio-graceful-shutdown
```

To run one of the examples (here `01_normal_shutdown.rs`), simply clone the [tokio-graceful-shutdown repository](https://github.com/Finomnis/tokio-graceful-shutdown), enter the repository folder and execute:
```bash
cargo run --example 01_normal_shutdown
```


## Motivation

Performing a graceful shutdown on an asynchronous program is a non-trivial problem. There are several solutions, but they all have their drawbacks:

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

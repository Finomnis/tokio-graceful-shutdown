# tokio-graceful-shutdown

IMPORTANT: This crate is in an early stage and not ready for production.

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
struct Subsystem1 {}

#[async_trait]
impl AsyncSubsystem for Subsystem1 {
    async fn run(mut self, subsys: SubsystemHandle)
      -> Result<()>
    {
        log::info!("Subsystem1 started.");
        subsys.on_shutdown_requested().await;
        log::info!("Subsystem1 stopped.");
        Ok(())
    }
}
```

This shows a simple asynchronous subsystem that simply starts, waits for the system shutdown to be triggered, and then stops itself.

This subsystem can now be executed like this:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    Toplevel::new()
        .start("Subsys1", Subsystem1::new())
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
```

The `Toplevel` object is the root object of the subsystem tree.
Subsystems can then be started using the `start()` functionality of the toplevel object.

The `catch_signals()` method signals the `Toplevel` object to listen for SIGINT/SIGTERM/Ctrl+C and initiate a shutdown thereafter.

`wait_for_shutdown()` is the final and most important method of `Toplevel`. It idles until the system enters the shutdown mode. Then, it collects all the return values of the subsystems and determines the global error state, and makes sure shutdown happens within the given timeout.
Lastly, it returns an error value that can be directly used as a return code for `main()`.

Further examples can be seen in the **examples** folder.

## Building

To use this library in your project, add the following to the `[dependencies]` section of `Cargo.toml`:
```toml
[dependencies]
tokio-graceful-shutdown = "0.2"
```

To run one of the examples (here `01_normal_shutdown.rs`), simply enter the repository folder and execute:
```bash
cargo run --example 01_normal_shutdown
```


## Motivation

Performing a graceful shutdown on an asynchronous system is a non-trivial problem. There are several solutions, but they all have their drawbacks:

- Global cancellation by forking with `tokio::select`. This is a wide-spread solution, but has the drawback that the canceled tasks cannot react to it, so it's impossible for them to shut down gracefully.
- Forking with `tokio::spawn` and signalling the desire to shutdown running tasks with mechanisms like `tokio::CancellationToken`. This allows tasks to shut down gracefully, but requires a lot of boilerplate code:
  - Passing the tokens to the tasks
  - Waiting for the tasks to finish
  - Implement a timeout mechanism to prevent deadlock

  If then further functionality is required, like listening for signals like SIGINT or SIGTERM, the boilerplate code will become quite messy.

And this is exactly what this crate aims to provide: clean abstractions to all this boilerplate code.


## Contributions

Contributions are welcome!

I primarily wrote this crate for my own convenience, so any ideas for improvements are
greatly appreciated.

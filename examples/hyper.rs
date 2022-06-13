//! This example demonstrates how to gracefully shutdown a hyper
//! server using this crate.
//!
//! This example closely follows hyper's "hello" example.
//!
//! Note that we have to wait for a long time in `handle_shutdown_requests` because
//! hyper's graceful shutdown waits for all connections to be closed naturally
//! instead of terminating them.

use env_logger::{Builder, Env};
use miette::{miette, Result};
use tokio::time::Duration;
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

use std::convert::Infallible;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

async fn hello(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("Hello World!")))
}

async fn hyper_subsystem(subsys: SubsystemHandle) -> Result<()> {
    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(|_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        async { Ok::<_, Infallible>(service_fn(hello)) }
    });

    let addr = ([127, 0, 0, 1], 12345).into();
    let server = Server::bind(&addr).serve(make_svc);

    log::info!("Listening on http://{}", addr);

    // This is the connection between our crate and hyper.
    // Hyper already anticipated our use case and provides a very
    // convenient inverface.
    server
        .with_graceful_shutdown(subsys.on_shutdown_requested())
        .await
        .or_else(|err| Err(miette! {err}))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Hyper", hyper_subsystem)
        .catch_signals()
        .handle_shutdown_requests(Duration::from_secs(60 * 5))
        .await
        .map_err(Into::into)
}

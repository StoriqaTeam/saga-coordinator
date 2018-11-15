extern crate chrono;
extern crate config as config_crate;
extern crate env_logger;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate futures_cpupool;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
extern crate tokio_signal;
extern crate tokio_timer;
extern crate uuid;
extern crate validator;
#[macro_use]
extern crate sentry;

extern crate stq_api;
extern crate stq_http;
extern crate stq_logging;
extern crate stq_router;
extern crate stq_routes;
extern crate stq_static_resources;
extern crate stq_types;

#[macro_use]
mod macros;
pub mod config;
mod controller;
mod errors;
mod microservice;
mod models;
pub mod sentry_integration;
mod services;

use std::process;
use std::sync::Arc;

use stq_http::controller::Application;

use futures::prelude::*;
use hyper::server::Http;
use tokio_core::reactor::Core;

use controller::ControllerImpl;
use errors::Error;

/// Starts new web service from provided `Config`
pub fn start_server(config: config::Config) {
    // Prepare server
    let address = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .expect("Could not parse address");

    // Prepare reactor
    let mut core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let client = stq_http::client::Client::new(&config.to_http_config(), &handle);

    let client_handle = client.handle();
    let client_stream = client.stream();
    handle.spawn(client_stream.for_each(|_| Ok(())));

    let serve = Http::new()
        .serve_addr_handle(&address, &*handle, {
            move || {
                // Prepare application
                let app = Application::<Error>::new(ControllerImpl {
                    config: config.clone(),
                    http_client: client_handle.clone(),
                    route_parser: Arc::new(controller::routes::create_route_parser()),
                });

                Ok(app)
            }
        }).unwrap_or_else(|reason| {
            eprintln!("Http Server Initialization Error: {}", reason);
            process::exit(1);
        });

    handle.spawn(
        serve
            .for_each({
                let handle = handle.clone();
                move |conn| {
                    handle.spawn(conn.map(|_| ()).map_err(|why| eprintln!("Server Error: {:?}", why)));
                    Ok(())
                }
            }).map_err(|_| ()),
    );

    info!("Listening on http://{}", address);
    core.run(tokio_signal::ctrl_c().flatten_stream().take(1u64).for_each(|()| {
        info!("Ctrl+C received. Exit");
        Ok(())
    })).unwrap();
}

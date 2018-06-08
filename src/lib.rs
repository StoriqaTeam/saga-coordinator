extern crate chrono;
extern crate config as config_crate;
extern crate stq_http;
extern crate stq_router;
extern crate stq_routes;

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
extern crate uuid;
extern crate validator;

pub mod config;
mod controller;
mod errors;
mod models;
mod services;

use std::env;
use std::io::Write;
use std::process;
use std::sync::Arc;

use stq_http::client::Client as HttpClient;
use stq_http::controller::Application;

use chrono::prelude::*;
use env_logger::Builder as LogBuilder;
use futures::future;
use futures::prelude::*;
use hyper::server::Http;
use log::LevelFilter as LogLevelFilter;
use tokio_core::reactor::Core;

use controller::ControllerImpl;
use errors::Error;

/// Starts new web service from provided `Config`
pub fn start_server(config: config::Config) {
    let mut builder = LogBuilder::new();
    builder
        .format(|formatter, record| {
            let now = Utc::now();
            writeln!(formatter, "{} - {} - {}", now.to_rfc3339(), record.level(), record.args())
        })
        .filter(None, LogLevelFilter::Info);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    // Prepare logger
    builder.init();

    let address = config.listen;

    // Prepare reactor
    let mut core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let client = HttpClient::new(
        &stq_http::client::Config {
            http_client_retries: 3,
            http_client_buffer_size: 10,
        },
        &(*handle).clone(),
    );
    let client_handle = Arc::new(client.handle());
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
        })
        .unwrap_or_else(|reason| {
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
            })
            .map_err(|_| ()),
    );

    info!("Listening on http://{}", address);
    core.run(future::empty::<(), ()>()).unwrap();
}

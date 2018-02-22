#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate stq_http;
extern crate stq_router;
extern crate stq_routes;

extern crate env_logger;
#[macro_use]
extern crate failure;
extern crate futures_await as futures;
extern crate futures_cpupool;
extern crate hyper;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
extern crate uuid;

pub mod config;
mod controller;
mod ops;

use stq_http::client::Client as HttpClient;
use stq_http::controller::{Application, Controller};
use stq_http::errors::ControllerError;
use stq_http::request_util::{read_body, ControllerFuture};
use stq_routes::model::Model as StqModel;
use stq_routes::role::Role as StqRole;
use stq_routes::role::UserRole as StqUserRole;
use stq_routes::role::NewUserRole as StqNewUserRole;
use stq_routes::service::Service as StqService;

use futures::prelude::*;
use futures_cpupool::CpuPool;
use hyper::{Method, StatusCode, Uri};
use hyper::client::{Client, HttpConnector};
use hyper::server::{Http, Request, Response, Service};
use std::collections::HashSet;
use std::sync::Arc;
use std::process;
use tokio_core::reactor::Core;

pub struct ControllerImpl {
    pub config: config::Config,
    pub http_client: Arc<HttpClient>,
}

impl Controller for ControllerImpl {
    fn call(&self, req: Request) -> ControllerFuture {
        match (req.method(), req.path()) {
            (&Method::Post, "create_account") => Box::new(
                read_body(req.body())
                    .map_err(|e| ControllerError::UnprocessableEntity(e.into()))
                    .and_then({
                        let http_client = self.http_client.clone();
                        let config = self.config.clone();
                        move |s| {
                            ops::account::create(http_client.clone(), config.clone(), s)
                                .map_err(|e| ControllerError::InternalServerError(e))
                        }
                    }),
            ),
            _ => Box::new(futures::future::err(ControllerError::NotFound)),
        }
    }
}

/// Starts new web service from provided `Config`
pub fn start_server(config: config::Config) {
    // Prepare logger
    env_logger::init();

    let address = "127.0.0.1:8080".parse().unwrap();
    let thread_count = 8;

    // Prepare reactor
    let core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let serve = Http::new()
        .serve_addr_handle(&address, &*handle, {
            let handle = handle.clone();
            move || {
                // Prepare application
                let app = Application {
                    controller: Box::new(ControllerImpl {
                        config: config.clone(),
                        http_client: Arc::new(HttpClient::new(
                            &stq_http::client::Config {
                                http_client_retries: 3,
                                http_client_buffer_size: 10,
                            },
                            &(*handle).clone(),
                        )),
                    }),
                };

                Ok(app)
            }
        })
        .unwrap_or_else(|reason| {
            eprintln!("Http Server Initialization Error: {}", reason);
            process::exit(1);
        });
}

#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate stq_http;
extern crate stq_router;

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

mod config;
mod controller;
mod create_account;

use stq_http::Client as HttpClient;
use stq_http::controller::{Application, Controller};
use stq_http::errors::ControllerError;
use stq_http::request_util::ControllerFuture;
use stq_router::model::Model as StqModel;
use stq_router::role::Role as StqRole;
use stq_router::role::UserRole as StqUserRole;
use stq_router::role::NewUserRole as StqNewUserRole;
use stq_router::service::Service as StqService;

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
            (&Method::Post, "create_account") => match req.body_ref() {
                None => Box::new(futures::future::err(ControllerError::Parse(
                    "No body".to_string(),
                ))),
                Some(body) => Box::new(
                    stq_http::request_util::read_body(*body.clone()).and_then(|s| {
                        create_account::op(self.http_client.clone(), self.config.clone(), s.to_string())
                            .map_err(|e| ControllerError::InternalServerError(e.to_string()))
                    }),
                ),
            },
            _ => Box::new(futures::future::err(futures::future::err(
                ControllerError::NotFound,
            ))),
        }
    }
}

/// Starts new web service from provided `Config`
pub fn start_server(config: config::Config) {
    // Prepare logger
    env_logger::init();

    let address = "127.0.0.1:8080";
    let thread_count = 8;

    // Prepare reactor
    let mut core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let serve = Http::new()
        .serve_addr_handle(&address, &handle, move || {
            // Prepare CPU pool
            let cpu_pool = CpuPool::new(thread_count);

            // Prepare application
            let app = SagaService {
                config,
                http_client: Arc::new(HttpClient::new()),
            };

            Ok(app)
        })
        .unwrap_or_else(|reason| {
            eprintln!("Http Server Initialization Error: {}", reason);
            process::exit(1);
        });
}

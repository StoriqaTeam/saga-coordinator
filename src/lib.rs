#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate env_logger;
extern crate failure;
extern crate futures_await as futures;
extern crate futures_cpupool;
extern crate hyper;
extern crate tokio_core;

use futures::future::Future;
use futures_cpupool::CpuPool;
use hyper::{Method, StatusCode};
use hyper::client::{Client, HttpConnector};
use hyper::header::ContentLength;
use hyper::server::{Http, Request, Response, Service};
use std::sync::Arc;
use tokio_core::reactor::Core;

#[derive(Clone, Debug)]
pub struct Config {
    pub users_addr: String,
    pub stores_addr: String,
}

type HClient = Client<HttpConnector>;

// Contains happy path for account creation
#[async]
fn create_account_happy(http_client: Arc<HClient>, config: Config, email: String, password: String) -> Result<(), failure::Error> {
    let res_create = await!(http_client.get(format!("{}/create_user", users_addr)))?;

    let res_set_store_role = await!(http_client.get(format!("{}/set_role", stores_addr)))?;

    Ok(())
}

// Contains reversal of account creation
#[async]
fn create_account_revert(http_client: Arc<HClient>, config: Config, email: String) -> Result<(), failure::Error> {
    Ok(())
}

#[async]
fn create_account_request(http_client: Arc<HClient>, config: Config, email: String, password: String) -> Result<(), failure::Error> {
    if let Err(e) = create_account_happy(http_client, config, email, password) {
        create_account_revert(http_client, config, email)
    } else {
        Ok(())
    }
}

pub struct SagaService {
    pub config: Config,
    pub http_client: Arc<HClient>,
}

impl Service for SagaService {
    // boilerplate hooking up hyper's server types
    type Request = Request;
    type Response = Response;
    type Error = failure::Error;
    // The future representing the eventual Response your call will
    // resolve to. This can change to whatever Future you need.
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Method::Post, "create_account") => Box::new(create_account_request(
                self.http_client.clone(),
                self.config.clone(),
                "test_email@yandex.ru".into(),
                "password".into(),
            )),
            _ => Box::new(futures::future::ok(
                Response::new().with_status(StatusCode::NotFound),
            )),
        }
    }
}

/// Starts new web service from provided `Config`
pub fn start_server(config: Config) {
    // Prepare logger
    env_logger::init().unwrap();

    // Prepare reactor
    let mut core = Core::new().expect("Unexpected error creating event loop core");
    let handle = Arc::new(core.handle());

    let serve = Http::new()
        .serve_addr_handle(&address, &handle, move || {
            // Prepare CPU pool
            let cpu_pool = CpuPool::new(thread_count);

            // Prepare application
            let app = SagaService;

            Ok(app)
        })
        .unwrap_or_else(|reason| {
            error!("Http Server Initialization Error: {}", reason);
            process::exit(1);
        });
}

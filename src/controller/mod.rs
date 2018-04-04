pub mod routes;


use std::sync::Arc;
use stq_router::RouteParser;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::controller::{Controller};
use stq_http::errors::ControllerError;
use stq_http::request_util::{read_body, ControllerFuture};
use stq_http::request_util::serialize_future;

use futures::prelude::*;
use futures::future;
use hyper::{Method};
use hyper::server::{ Request};

use config::Config;
use self::routes::Route;
use ops;


pub struct ControllerImpl {
    pub config: Config,
    pub http_client: Arc<HttpClientHandle>,
    pub route_parser: Arc<RouteParser<Route>>,
}

impl Controller for ControllerImpl {
    fn call(&self, req: Request) -> ControllerFuture {
        match (req.method(), self.route_parser.test(req.path())) {

            (&Method::Post, Some(Route::CreateAccount)) => serialize_future(
                read_body(req.body())
                    .map_err(|e| ControllerError::UnprocessableEntity(e.into()))
                    .and_then({
                        let http_client = self.http_client.clone();
                        let config = self.config.clone();
                        println!("Create account");
                        move |s| {
                            ops::account::create(http_client.clone(), config.clone(), s)
                                .map_err(|e| ControllerError::InternalServerError(e))
                        }
                    }),
            ),
            _ => Box::new(future::err(ControllerError::NotFound)),
        }
    }
}
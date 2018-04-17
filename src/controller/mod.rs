pub mod routes;

use std::collections::HashMap;

use std::sync::Arc;
use stq_router::RouteParser;

use serde_json;
use stq_http::client::Error;
use stq_http::errors::ErrorMessage;
use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::controller::Controller;
use stq_http::errors::ControllerError;
use stq_http::request_util::{read_body, ControllerFuture};
use stq_http::request_util::serialize_future;

use futures::prelude::*;
use futures::future;
use hyper::Method;
use hyper::server::Request;

use config::Config;
use self::routes::Route;
use ops;

use validator::{ValidationError, ValidationErrors};

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
                            ops::account::create(http_client.clone(), config.clone(), s).map_err(|e| match e {
                                Error::Api(status, Some(ErrorMessage { code, message })) => {
                                    let _status = status;
                                    let _code = code;
                                    let message = message.to_string();
                                    // Wierd construction of ValidationErrors dou to the fact ValidationErrors.add
                                    // only accepts str with static lifetime
                                    let valid_err_res = serde_json::from_str::<HashMap<&str, Vec<ValidationError>>>(&message);
                                    match valid_err_res {
                                        Ok(valid_err_map) => {
                                            let mut valid_errors = ValidationErrors::new();

                                            if let Some(map_val) = valid_err_map.get("email") {
                                                if !map_val.is_empty() {
                                                    valid_errors.add("email", map_val[0].clone())
                                                }
                                            }

                                            if let Some(map_val) = valid_err_map.get("password") {
                                                if !map_val.is_empty() {
                                                    valid_errors.add("password", map_val[0].clone())
                                                }
                                            }

                                            ControllerError::Validate(valid_errors)
                                        }
                                        Err(_) => ControllerError::InternalServerError(format_err!("Unknown")),
                                    }
                                }
                                _ => ControllerError::InternalServerError(format_err!("Unknown")),
                            })
                        }
                    }),
            ),
            _ => Box::new(future::err(ControllerError::NotFound)),
        }
    }
}

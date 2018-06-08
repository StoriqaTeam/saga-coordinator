pub mod routes;

use std::collections::HashMap;
use std::sync::Arc;

use failure::Error as FailureError;
use failure::Fail;
use futures::future;
use futures::prelude::*;
use hyper::Method;
use hyper::server::Request;
use serde_json;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::client::Error as HttpError;
use stq_http::controller::Controller;
use stq_http::errors::ErrorMessage;
use stq_http::request_util::serialize_future;
use stq_http::request_util::parse_body;
use stq_http::controller::ControllerFuture;
use stq_router::RouteParser;

use self::routes::Route;
use config::Config;
use models::SagaCreateProfile;
use services::system::{SystemService, SystemServiceImpl};
use services::account::{AccountServiceImpl, AccountService};
use errors::Error;

use validator::{ValidationError, ValidationErrors};

pub struct ControllerImpl {
    pub config: Config,
    pub http_client: Arc<HttpClientHandle>,
    pub route_parser: Arc<RouteParser<Route>>,
}

impl Controller for ControllerImpl {
    fn call(&self, req: Request) -> ControllerFuture {
        let system_service = SystemServiceImpl::new();
        let http_client = self.http_client.clone();
        let config = self.config.clone();
        let account_service = AccountServiceImpl::new(http_client, config);

        match (&req.method().clone(), self.route_parser.test(req.path().clone())) {
            // GET /healthcheck
            (&Method::Get, Some(Route::Healthcheck)) => {
                trace!("Received healthcheck request");
                serialize_future(system_service.healthcheck())
            }

            (&Method::Post, Some(Route::CreateAccount)) => serialize_future(
                parse_body::<SagaCreateProfile>(req.body())
                    .map_err(|e| {
                        e.context("Parsing body // POST /stores/search in SearchStore failed!")
                            .context(Error::Parse)
                            .into()
                    })
                    .and_then({
                        move |s| {
                            account_service.create(s).map_err(|e| match e {
                                HttpError::Api(status, Some(ErrorMessage { payload, .. })) => {
                                    let payload = payload.to_string();
                                    // Wierd construction of ValidationErrors dou to the fact ValidationErrors.add
                                    // only accepts str with static lifetime
                                    let valid_err_res = serde_json::from_str::<HashMap<&str, Vec<ValidationError>>>(&payload);
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

                                            Error::Validate(valid_errors).into()
                                        }
                                        Err(e) => {
                                            e.context("Cannot parse validation errors").into()
                                        }
                                    }
                                }
                                e => e.into(),
                            })
                        }
                    })
                    .map_err(|e: FailureError| e.context("Error during account creation in saga occured.").into())
            ),
            
            // Fallback
            (m, r) => {
                Box::new(future::err(
                    Error::NotFound
                        .context(format!("Request to non existing endpoint in saga microservice! {:?} {:?}", m, r))
                        .into(),
                ))
            }
        }
    }
}

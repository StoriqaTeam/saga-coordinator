pub mod routes;

use std::collections::HashMap;
use std::sync::Arc;

use failure::Error as FailureError;
use failure::Fail;
use futures::future;
use futures::prelude::*;
use hyper::server::Request;
use hyper::Method;
use serde_json;
use validator::{ValidationError, ValidationErrors};

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::client::Error as HttpError;
use stq_http::controller::Controller;
use stq_http::controller::ControllerFuture;
use stq_http::errors::ErrorMessage;
use stq_http::request_util::parse_body;
use stq_http::request_util::serialize_future;
use stq_router::RouteParser;

use self::routes::Route;
use config::Config;
use errors::Error;
use models::SagaCreateProfile;
use services::account::{AccountService, AccountServiceImpl};

pub struct ControllerImpl {
    pub config: Config,
    pub http_client: Arc<HttpClientHandle>,
    pub route_parser: Arc<RouteParser<Route>>,
}

impl Controller for ControllerImpl {
    fn call(&self, req: Request) -> ControllerFuture {
        let http_client = self.http_client.clone();
        let config = self.config.clone();
        let account_service = AccountServiceImpl::new(http_client, config);
        let path = req.path().to_string();

        match (&req.method().clone(), self.route_parser.test(req.path())) {
            (&Method::Post, Some(Route::CreateAccount)) => serialize_future(
                parse_body::<SagaCreateProfile>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /create_account in SagaCreateProfile failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |profile| {
                        account_service
                            .create(profile)
                            .map_err(|(_, e)| {
                                {
                                    if let Some(Error::HttpClient(HttpError::Api(_, Some(ErrorMessage { payload, .. })))) =
                                        e.downcast_ref::<Error>()
                                    {
                                        if let Some(payload) = payload {
                                            // Wierd construction of ValidationErrors dou to the fact ValidationErrors.add
                                            // only accepts str with static lifetime
                                            let valid_err_res =
                                                serde_json::from_value::<HashMap<String, Vec<ValidationError>>>(payload.clone());
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

                                                    return Error::Validate(valid_errors).into();
                                                }
                                                Err(e) => {
                                                    return e.context("Cannot parse validation errors").into();
                                                }
                                            }
                                        } else {
                                            return format_err!("Http error does not contain payload. ").into();
                                        }
                                    }
                                }
                                e
                            })
                            .map(|(_, user)| user)
                    })
                    .map_err(|e: FailureError| FailureError::from(e.context("Error during account creation in saga occured."))),
            ),

            // Fallback
            (m, _) => Box::new(future::err(
                format_err!("Request to non existing endpoint in saga coordinator microservice! {:?} {:?}", m, path)
                    .context(Error::NotFound)
                    .into(),
            )),
        }
    }
}

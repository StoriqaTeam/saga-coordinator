//! `Controller` is a top layer that handles all http-related
//! stuff like reading bodies, parsing params, forming a response.
//! Basically it provides inputs to `Service` layer and converts outputs
//! of `Service` layer to http responses
pub mod routes;

use std::str::FromStr;
use std::sync::Arc;

use failure::Error as FailureError;
use failure::Fail;
use futures::future;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::server::Request;
use hyper::Method;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::controller::Controller;
use stq_http::controller::ControllerFuture;
use stq_http::request_util::parse_body;
use stq_http::request_util::serialize_future;
use stq_router::RouteParser;

use self::routes::Route;
use config::Config;
use errors::Error;
use models::{ConvertCart, NewStore, SagaCreateProfile};
use services::account::{AccountService, AccountServiceImpl};
use services::order::{OrderService, OrderServiceImpl};
use services::store::{StoreService, StoreServiceImpl};

pub struct ControllerImpl {
    pub config: Config,
    pub http_client: Arc<HttpClientHandle>,
    pub route_parser: Arc<RouteParser<Route>>,
}

impl Controller for ControllerImpl {
    fn call(&self, req: Request) -> ControllerFuture {
        let headers = req.headers().clone();
        let auth_header = headers.get::<Authorization<String>>();
        let user_id = auth_header.map(|auth| auth.0.clone()).and_then(|id| i32::from_str(&id).ok());

        let http_client = self.http_client.clone();
        let config = self.config.clone();
        let account_service = AccountServiceImpl::new(http_client.clone(), config.clone());
        let store_service = StoreServiceImpl::new(http_client.clone(), config.clone(), user_id);
        let order_service = OrderServiceImpl::new(http_client, config, user_id);
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
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during account creation in saga occured.")))
                    }),
            ),

            (&Method::Post, Some(Route::CreateStore)) => serialize_future(
                parse_body::<NewStore>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /create_store in NewStore failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |store| {
                        store_service
                            .create(store)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during store creation in saga occured.")))
                    }),
            ),

            (&Method::Post, Some(Route::CreateOrder)) => serialize_future(
                parse_body::<ConvertCart>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /create_order in ConvertCart failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |new_order| {
                        order_service
                            .create(new_order)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during order creation in saga occured.")))
                    }),
            ),

            // Fallback
            (m, _) => Box::new(future::err(
                format_err!(
                    "Request to non existing endpoint in saga coordinator microservice! {:?} {:?}",
                    m,
                    path
                ).context(Error::NotFound)
                    .into(),
            )),
        }
    }
}

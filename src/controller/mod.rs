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
use stq_types::UserId;

use self::routes::Route;
use config::Config;
use errors::Error;
use models::{
    BillingOrdersVec, ConvertCart, EmailVerifyApply, NewStore, PasswordResetApply, ResetRequest, SagaCreateProfile, UpdateStatePayload,
};
use services::account::{AccountService, AccountServiceImpl};
use services::order::{OrderService, OrderServiceImpl};
use services::store::{StoreService, StoreServiceImpl};

pub struct ControllerImpl {
    pub config: Config,
    pub http_client: HttpClientHandle,
    pub route_parser: Arc<RouteParser<Route>>,
}

impl Controller for ControllerImpl {
    fn call(&self, req: Request) -> ControllerFuture {
        let headers = req.headers().clone();
        let auth_header = headers.get::<Authorization<String>>();
        let user_id = auth_header
            .map(|auth| auth.0.clone())
            .and_then(|id| i32::from_str(&id).ok())
            .map(UserId);

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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during account creation occured.")))
                    }),
            ),
            (&Method::Post, Some(Route::VerifyEmail)) => serialize_future(
                parse_body::<ResetRequest>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /email_verify in ResetRequest failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |profile| {
                        account_service
                            .request_email_verification(profile)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during email verification occured.")))
                    }),
            ),
            (&Method::Post, Some(Route::VerifyEmailApply)) => serialize_future(
                parse_body::<EmailVerifyApply>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /email_verify_apply in EmailVerifyApply failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |profile| {
                        account_service
                            .request_email_verification_apply(profile)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during email verification apply occured.")))
                    }),
            ),
            (&Method::Post, Some(Route::ResetPassword)) => serialize_future(
                parse_body::<ResetRequest>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /reset_password in ResetRequest failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |profile| {
                        account_service
                            .request_password_reset(profile)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during reset password occured.")))
                    }),
            ),
            (&Method::Post, Some(Route::ResetPasswordApply)) => serialize_future(
                parse_body::<PasswordResetApply>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /reset_password_apply in PasswordResetApply failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |profile| {
                        account_service
                            .request_password_reset_apply(profile)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during reset password apply occured.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during store creation occured.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during order creation occured.")))
                    }),
            ),

            (&Method::Post, Some(Route::OrdersUpdateStateByBilling)) => serialize_future(
                parse_body::<BillingOrdersVec>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /orders/update_state in BillingOrdersVec failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |orders_info| {
                        order_service
                            .update_state_by_billing(orders_info)
                            .map(|(_, _)| ())
                            .map_err(|(_, e)| FailureError::from(e.context("Error during orders update by external billing occured.")))
                    }),
            ),

            (&Method::Post, Some(Route::OrdersManualSetState { order_slug })) => serialize_future(
                parse_body::<UpdateStatePayload>(req.body())
                    .map_err(move |e| {
                        FailureError::from(
                            e.context(format!(
                                "Parsing body // POST /orders/{}/set_state in UpdateStatePayload failed!",
                                order_slug
                            )).context(Error::Parse),
                        )
                    })
                    .and_then(move |payload| {
                        order_service
                            .manual_set_state(order_slug, payload.state, payload.track_id, payload.comment)
                            .map(|(_, _)| ())
                            .map_err(|(_, e)| FailureError::from(e.context("Error during orders manual update occured.")))
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

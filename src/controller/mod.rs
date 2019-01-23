//! `Controller` is a top layer that handles all http-related
//! stuff like reading bodies, parsing params, forming a response.
//! Basically it provides inputs to `Service` layer and converts outputs
//! of `Service` layer to http responses
pub mod requests;
pub mod routes;

use std::sync::Arc;
use std::time::Duration;

use failure::Error as FailureError;
use failure::Fail;
use futures::future;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::header::Headers;
use hyper::server::Request;
use hyper::Method;

use stq_http::client::{ClientHandle as HttpClientHandle, HttpClientWithDefaultHeaders, TimeLimitedHttpClient};
use stq_http::controller::Controller;
use stq_http::controller::ControllerFuture;
use stq_http::errors::ErrorMessageWrapper;
use stq_http::request_util::parse_body;
use stq_http::request_util::serialize_future;
use stq_http::request_util::CorrelationToken as CorrelationTokenHeader;
use stq_http::request_util::Currency as CurrencyHeader;
use stq_http::request_util::RequestTimeout as RequestTimeoutHeader;
use stq_router::RouteParser;

use self::routes::Route;
use config::Config;
use errors::Error;
use microservice::{
    BillingMicroserviceImpl, DeliveryMicroserviceImpl, NotificationsMicroserviceImpl, OrdersMicroserviceImpl, StoresMicroserviceImpl,
    UsersMicroserviceImpl, WarehousesMicroserviceImpl,
};
use models::*;
use sentry_integration::log_and_capture_error;
use services::account::{AccountService, AccountServiceImpl};
use services::delivery::{DeliveryService, DeliveryServiceImpl};
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

        let default_timeout = Duration::from_millis(self.config.client.http_timeout_ms);
        let request_timeout = match headers.get::<RequestTimeoutHeader>() {
            None => default_timeout,
            Some(header) => header.0.parse::<u64>().map(Duration::from_millis).unwrap_or(default_timeout),
        }
        .checked_sub(Duration::from_millis(self.config.service.processing_timeout_ms))
        .unwrap_or(Duration::new(0, 0));

        let http_client = TimeLimitedHttpClient::new(self.http_client.clone(), request_timeout);

        let orders_microservice = Arc::new(OrdersMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), default_headers(&headers)),
            self.config.clone(),
        ));

        let stores_microservice = Arc::new(StoresMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), stores_headers(&headers)),
            self.config.clone(),
        ));

        let notifications_microservice = Arc::new(NotificationsMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), default_headers(&headers)),
            self.config.clone(),
        ));

        let users_microservice = Arc::new(UsersMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), default_headers(&headers)),
            self.config.clone(),
        ));

        let billing_microservice = Arc::new(BillingMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), default_headers(&headers)),
            self.config.clone(),
        ));

        let warehouses_microservice = Arc::new(WarehousesMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), default_headers(&headers)),
            self.config.clone(),
        ));

        let delivery_microservice = Arc::new(DeliveryMicroserviceImpl::new(
            HttpClientWithDefaultHeaders::new(http_client.clone(), default_headers(&headers)),
            self.config.clone(),
        ));

        let config = self.config.clone();

        let account_service = AccountServiceImpl::new(
            config.clone(),
            stores_microservice.clone(),
            billing_microservice.clone(),
            delivery_microservice.clone(),
            users_microservice.clone(),
            notifications_microservice.clone(),
        );
        let store_service = StoreServiceImpl::new(
            config.clone(),
            orders_microservice.clone(),
            stores_microservice.clone(),
            notifications_microservice.clone(),
            billing_microservice.clone(),
            warehouses_microservice.clone(),
            users_microservice.clone(),
            delivery_microservice.clone(),
        );

        let order_service = OrderServiceImpl::new(
            config.clone(),
            orders_microservice.clone(),
            stores_microservice.clone(),
            notifications_microservice.clone(),
            users_microservice.clone(),
            billing_microservice.clone(),
            warehouses_microservice.clone(),
        );

        let delivery_service = DeliveryServiceImpl::new(
            config,
            orders_microservice.clone(),
            delivery_microservice.clone(),
            stores_microservice.clone(),
        );

        let path = req.path().to_string();

        let fut = match (&req.method().clone(), self.route_parser.test(req.path())) {
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during account creation occurred.")))
                    }),
            ),
            (&Method::Post, Some(Route::VerifyEmail)) => serialize_future(
                parse_body::<VerifyRequest>(req.body())
                    .map_err(|e| {
                        FailureError::from(
                            e.context("Parsing body // POST /email_verify in VerifyRequest failed!")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |profile| {
                        account_service
                            .request_email_verification(profile)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during email verification occurred.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during email verification apply occurred.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during reset password occurred.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during reset password apply occurred.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during store creation occurred.")))
                    }),
            ),

            (&Method::Post, Some(Route::CreateOrder)) => serialize_future(
                parse_body::<ConvertCart>(req.body())
                    .map_err(|e| FailureError::from(e.context("Parsing body failed, target: ConvertCart").context(Error::Parse)))
                    .and_then(move |new_order| {
                        order_service
                            .create(new_order)
                            .map(|(_, user)| user)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during order creation occurred.")))
                    }),
            ),

            (&Method::Post, Some(Route::BuyNow)) => serialize_future(
                parse_body::<BuyNow>(req.body())
                    .map_err(|e| FailureError::from(e.context("Parsing body // POST /buy_now in BuyNow failed!").context(Error::Parse)))
                    .and_then(move |new_buy_now| {
                        order_service
                            .create_buy_now(new_buy_now)
                            .map(|(_, invoice)| invoice)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during order creation from buy now data occurred.")))
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
                            .map_err(|(_, e)| FailureError::from(e.context("Error during orders update by external billing occurred.")))
                    }),
            ),

            (&Method::Post, Some(Route::OrdersManualSetState { order_slug })) => serialize_future(
                parse_body::<UpdateStatePayload>(req.body())
                    .map_err(move |e| {
                        FailureError::from(
                            e.context(format!(
                                "Parsing body // POST /orders/{}/set_state in UpdateStatePayload failed!",
                                order_slug
                            ))
                            .context(Error::Parse),
                        )
                    })
                    .and_then(move |payload| {
                        order_service
                            .manual_set_state(order_slug, payload.state, payload.track_id, payload.comment, payload.committer_role)
                            .map(|(_, order)| order)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during orders manual update occurred.")))
                    }),
            ),

            (&Method::Post, Some(Route::OrdersSetPaymentState { order_id })) => serialize_future({
                parse_body::<OrderPaymentStateRequest>(req.body())
                    .map_err(move |e| {
                        FailureError::from(
                            e.context("Parsing body failed, target: OrderPaymentStateRequest")
                                .context(Error::Parse),
                        )
                    })
                    .and_then(move |payload| {
                        order_service
                            .manual_set_payment_state(order_id, payload)
                            .map(|_| ())
                            .map_err(|(_, e)| FailureError::from(e.context("Error during orders manual payment state update occurred.")))
                    })
            }),

            // POST /stores/moderate
            (&Method::Post, Some(Route::StoreModerate)) => serialize_future(
                parse_body::<StoreModerate>(req.body())
                    .map_err(|e| FailureError::from(e.context("Parsing body failed, target: StoreModerate").context(Error::Parse)))
                    .and_then(move |store_moderate| {
                        store_service
                            .set_store_moderation_status(store_moderate)
                            .map(|(_, store)| store)
                            .map_err(|(_, e)| FailureError::from(e.context("Error during change store status occurred.")))
                    }),
            ),

            // POST /stores/moderation
            (&Method::Post, Some(Route::StoreModeration(store_id))) => serialize_future(
                store_service
                    .send_to_moderation(store_id)
                    .map(|(_, store)| store)
                    .map_err(|(_, e)| FailureError::from(e.context("Error sending store to moderation occurred."))),
            ),

            // POST /stores/<store_id>/deactivate
            (&Method::Post, Some(Route::StoreDeactivate(store_id))) => serialize_future(
                store_service
                    .deactivate_store(store_id)
                    .map(|(_, store)| store)
                    .map_err(|(_, e)| FailureError::from(e.context("Error deactivating store occurred."))),
            ),

            // POST /base_products/moderate
            (&Method::Post, Some(Route::BaseProductModerate)) => serialize_future(
                parse_body::<BaseProductModerate>(req.body())
                    .map_err(|e| FailureError::from(e.context("Parsing body failed, target: BaseProductModerate").context(Error::Parse)))
                    .and_then(move |base_product_moderate| {
                        store_service
                            .set_moderation_status_base_product(base_product_moderate)
                            .map(|(_, _)| ())
                            .map_err(|(_, e)| FailureError::from(e.context("Error change base product status occurred.")))
                    }),
            ),

            // POST /base_products/moderation
            (&Method::Post, Some(Route::BaseProductModeration(base_product_id))) => serialize_future(
                store_service
                    .send_to_moderation_base_product(base_product_id)
                    .map(|(_, _)| ())
                    .map_err(|(_, e)| FailureError::from(e.context("Error sending base product to moderation occurred."))),
            ),

            // POST /base_products/<base_product_id>/deactivate
            (&Method::Post, Some(Route::BaseProductDeactivate(base_product_id))) => serialize_future(
                store_service
                    .deactivate_base_product(base_product_id)
                    .map(|(_, base_product)| base_product)
                    .map_err(|(_, e)| FailureError::from(e.context("Error deactivating base product occurred."))),
            ),

            // POST /base_products/<base_product_id>/upsert-shipping
            (&Method::Post, Some(Route::BaseProductUpsertShipping(base_product_id))) => serialize_future(
                parse_body::<NewShipping>(req.body())
                    .map_err(|e| FailureError::from(e.context("Parsing body failed, target: NewShipping").context(Error::Parse)))
                    .and_then(move |payload| {
                        delivery_service
                            .upsert_shipping(base_product_id, payload)
                            .map(|(_, shipping)| shipping)
                            .map_err(|(_, e)| FailureError::from(e.context("Error update shipping for base product occurred.")))
                    }),
            ),

            // POST /products/<product_id>/deactivate
            (&Method::Post, Some(Route::ProductDeactivate(product_id))) => serialize_future(
                store_service
                    .deactivate_product(product_id)
                    .map(|(_, product)| product)
                    .map_err(|(_, e)| FailureError::from(e.context("Error deactivating product occurred."))),
            ),

            // Fallback
            (m, _) => Box::new(future::err(
                format_err!(
                    "Request to non existing endpoint in saga coordinator microservice! {:?} {:?}",
                    m,
                    path
                )
                .context(Error::NotFound)
                .into(),
            )),
        }
        .map_err(|err| {
            let wrapper = ErrorMessageWrapper::<Error>::from(&err);
            if wrapper.inner.code == 500 {
                log_and_capture_error(&err);
            }
            err
        });

        Box::new(fut)
    }
}

fn default_headers(request_headers: &Headers) -> Headers {
    let mut headers = Headers::new();
    if let Some(auth) = request_headers.get::<Authorization<String>>() {
        headers.set(auth.clone());
    }
    if let Some(correlation) = request_headers.get::<CorrelationTokenHeader>() {
        headers.set(correlation.clone());
    }
    headers
}

fn stores_headers(request_headers: &Headers) -> Headers {
    let mut stores_headers = default_headers(request_headers);
    stores_headers.set(CurrencyHeader("STQ".to_string()));
    stores_headers
}

use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use futures;
use futures::future;
use futures::future::join_all;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;

use serde_json;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::SagaId;

use config;
use errors::Error;
use models::*;
use services::types::ServiceFuture;

pub trait OrderService {
    fn create(self, input: ConvertCart) -> ServiceFuture<Box<OrderService>, Invoice>;
    fn update_state(self, orders: BillingOrdersVec) -> Box<Future<Item = String, Error = FailureError>>;
}

/// Orders services, responsible for Creating orders
pub struct OrderServiceImpl {
    pub http_client: Arc<HttpClientHandle>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateOrderOperationLog>>,
    pub user_id: Option<i32>,
}

impl OrderServiceImpl {
    pub fn new(http_client: Arc<HttpClientHandle>, config: config::Config, user_id: Option<i32>) -> Self {
        let log = Arc::new(Mutex::new(CreateOrderOperationLog::new()));
        Self {
            http_client,
            config,
            log,
            user_id,
        }
    }

    fn convert_cart(self, input: ConvertCart) -> ServiceFuture<Self, Vec<Order>> {
        // Create Order
        debug!("Converting cart, input: {:?}", input);
        let convert_cart: ConvertCartWithConversionId = input.into();
        let convertion_id = convert_cart.conversion_id;
        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateOrderOperationStage::OrdersConvertCartStart(convertion_id));

        let mut headers = Headers::new();
        if let Some(ref user_id) = self.user_id {
            headers.set(Authorization(user_id.to_string()));
        };

        let orders_url = self.config.service_url(StqService::Orders);
        let client = self.http_client.clone();

        let res = serde_json::to_string(&convert_cart)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<Vec<Order>>(
                        Method::Post,
                        format!("{}/{}/create_from_cart", orders_url, StqModel::Order.to_url()),
                        Some(body),
                        Some(headers),
                    )
                    .map_err(|e| {
                        format_err!("Converting cart in orders microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateOrderOperationStage::OrdersConvertCartComplete(convertion_id));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_invoice(self, input: CreateInvoice) -> ServiceFuture<Self, Invoice> {
        // Create invoice
        debug!("Creating invoice, input: {}", input);
        let log = self.log.clone();

        let saga_id = input.saga_id;
        log.lock()
            .unwrap()
            .push(CreateOrderOperationStage::BillingCreateInvoiceStart(saga_id));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can create invoice

        let billing_url = self.config.service_url(StqService::Billing);
        let client = self.http_client.clone();

        let res = serde_json::to_string(&input)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<Invoice>(Method::Post, format!("{}/invoices", billing_url), Some(body), Some(headers))
                    .map_err(|e| {
                        format_err!("Creating invoice in billing microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateOrderOperationStage::BillingCreateInvoiceComplete(saga_id));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    // Contains happy path for Order creation
    fn create_happy(self, input: ConvertCart) -> ServiceFuture<Self, Invoice> {
        Box::new(self.convert_cart(input.clone()).and_then(move |(s, orders)| {
            let create_invoice = CreateInvoice {
                customer_id: input.customer_id,
                orders,
                currency_id: input.currency_id,
                saga_id: SagaId::new(),
            };
            s.create_invoice(create_invoice)
        }))
    }

    // Contains reversal of Order creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();

        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log.into_iter() {
            match e {
                CreateOrderOperationStage::OrdersConvertCartStart(conversion_id) => {
                    debug!("Reverting cart convertion, conversion_id: {}", conversion_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };

                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can revert orders

                        let payload = ConvertCartRevert { conversion_id };
                        let body = serde_json::to_string(&payload).unwrap_or_default();

                        s.http_client
                            .request::<CartHash>(
                                Method::Post,
                                format!(
                                    "{}/{}/create_from_cart/revert",
                                    s.config.service_url(StqService::Orders),
                                    StqModel::Order.to_url()
                                ),
                                Some(body),
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Order service create_revert OrdersConvertCartStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateOrderOperationStage::BillingCreateInvoiceStart(saga_id) => {
                    debug!("Reverting create invoice, saga_id: {}", saga_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can revert invoice

                        s.http_client
                            .request::<SagaId>(
                                Method::Delete,
                                format!(
                                    "{}/invoices/by-saga-id/{}",
                                    s.config.service_url(StqService::Billing),
                                    saga_id.0.clone(),
                                ),
                                None,
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Order service create_revert BillingCreateInvoiceStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                _ => {}
            }
        }

        fut
    }
}

impl OrderService for OrderServiceImpl {
    fn create(self, input: ConvertCart) -> ServiceFuture<Box<OrderService>, Invoice> {
        Box::new(
            self.create_happy(input.clone())
                .map(|(s, order)| (Box::new(s) as Box<OrderService>, order))
                .or_else(move |(s, e)| {
                    s.create_revert().then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        futures::future::err((Box::new(s) as Box<OrderService>, e.into()))
                    })
                }),
        )
    }

    fn update_state(self, orders_info: BillingOrdersVec) -> Box<Future<Item = String, Error = FailureError>> {
        debug!("Updating orders status: {}", orders_info);

        let client = self.http_client.clone();
        let orders_url = self.config.service_url(StqService::Orders);
        let notifications_url = self.config.service_url(StqService::Notifications);
        let users_url = self.config.service_url(StqService::Users);
        let stores_url = self.config.service_url(StqService::Stores);

        let mut orders_futures = vec![];
        for order_info in orders_info.0 {
            let payload: UpdateStatePayload = order_info.clone().into();
            let body = serde_json::to_string(&payload).unwrap_or_default();
            let url = format!("{}/{}/by-id/{}/status", orders_url, StqModel::Order.to_url(), order_info.order_id.0);
            let mut headers = Headers::new();
            headers.set(Authorization(order_info.customer_id.0.to_string()));
            let res = client
                .request::<Option<Order>>(Method::Put, url, Some(body.clone()), Some(headers))
                .map_err(|e| {
                    format_err!("Setting new status in orders microservice error occured.")
                        .context(Error::HttpClient(e))
                        .into()
                })
                .and_then({
                    let client = client.clone();
                    let order_id = order_info.order_id.0;
                    let customer_id = order_info.customer_id.0;
                    let store_id = order_info.store_id.0;
                    let users_url = users_url.clone();
                    let stores_url = stores_url.clone();
                    let notifications_url = notifications_url.clone();

                    move |order| {
                        if let Some(order) = order {
                            let url = format!("{}/{}/{}", users_url, StqModel::User.to_url(), order.customer_id);
                            let mut headers = Headers::new();
                            headers.set(Authorization(customer_id.to_string()));
                            let send_to_client = client
                                .request::<Option<User>>(Method::Get, url, None, Some(headers))
                                .map_err(From::from)
                                .and_then({
                                    let client = client.clone();
                                    let notifications_url = notifications_url.clone();
                                    move |user| {
                                        if let Some(user) = user {
                                            let to = user.email.clone();
                                            let subject = format!("Changed order {} state.", order_id);
                                            let text = format!(
                                                "Order {} has changed it's state. You can watch current order state on your orders page.",
                                                order_id
                                            );
                                            let url = format!("{}/sendmail", notifications_url);
                                            Box::new(
                                                serde_json::to_string(&ResetMail { to, subject, text })
                                                    .map_err(From::from)
                                                    .into_future()
                                                    .and_then(move |body| {
                                                        client.request::<String>(Method::Post, url, Some(body), None).map_err(From::from)
                                                    }),
                                            )
                                                as Box<Future<Item = String, Error = FailureError>>
                                        } else {
                                            error!(
                                                "Sending notification to user can not be done. User with id: {} is not found.",
                                                customer_id
                                            );
                                            Box::new(future::err(
                                                format_err!("User is not found in users microservice.")
                                                    .context(Error::NotFound)
                                                    .into(),
                                            ))
                                        }
                                    }
                                })
                                .map(|_| ());

                            let url = format!("{}/{}/{}", stores_url, StqModel::Store.to_url(), order.store_id);
                            let send_to_store = client
                                .request::<Option<Store>>(Method::Get, url, None, None)
                                .map_err(From::from)
                                .and_then({
                                    let client = client.clone();
                                    let notifications_url = notifications_url.clone();
                                    move |store| {
                                        if let Some(store) = store {
                                            if let Some(email) = store.email {
                                                let to = email;
                                                let subject = format!("Changed order {} state.", order_id);
                                                let text = format!(
                                                    "Order {} has changed it's state. You can watch current order state on its page.",
                                                    order_id
                                                );
                                                let url = format!("{}/sendmail", notifications_url);
                                                Box::new(
                                                    serde_json::to_string(&ResetMail { to, subject, text })
                                                        .map_err(From::from)
                                                        .into_future()
                                                        .and_then(move |body| {
                                                            client
                                                                .request::<String>(Method::Post, url, Some(body), None)
                                                                .map_err(From::from)
                                                        }),
                                                )
                                                    as Box<Future<Item = String, Error = FailureError>>
                                            } else {
                                                Box::new(future::ok(String::default()))
                                            }
                                        } else {
                                            error!(
                                                "Sending notification to store can not be done. Store with id: {} is not found.",
                                                store_id
                                            );
                                            Box::new(future::err(
                                                format_err!("Store is not found in stores microservice.")
                                                    .context(Error::NotFound)
                                                    .into(),
                                            ))
                                        }
                                    }
                                })
                                .map(|_| ());
                            Box::new(send_to_client.then(|_| send_to_store).then(|_| Ok(())))
                                as Box<Future<Item = (), Error = FailureError>>
                        } else {
                            Box::new(future::err(
                                format_err!("Order is not found in orders microservice! id: {}", order_info.order_id.0)
                                    .context(Error::NotFound)
                                    .into(),
                            ))
                        }
                    }
                });
            orders_futures.push(Box::new(res) as Box<Future<Item = (), Error = FailureError>>);
        }

        Box::new(
            join_all(orders_futures)
                .map(|_| "Ok".to_string())
                .map_err(|e: FailureError| e.context(format!("Setting new orders status error.")).into()),
        )
    }
}

use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures;
use futures::future;
use futures::future::join_all;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;
use serde_json;

use stq_api::orders::Order;
use stq_api::orders::OrderClient;
use stq_api::rpc_client::RestApiClient;
use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_static_resources::{
    EmailUser, OrderCreateForStore, OrderCreateForUser, OrderState, OrderUpdateStateForStore, OrderUpdateStateForUser,
};
use stq_types::{OrderSlug, SagaId, StoreId, UserId};

use super::parse_validation_errors;
use config;
use errors::Error;
use models::*;
use services::types::ServiceFuture;

pub trait OrderService {
    fn create(self, input: ConvertCart) -> ServiceFuture<Box<OrderService>, Invoice>;
    fn update_state(self, orders_info: BillingOrdersVec) -> ServiceFuture<Box<OrderService>, ()>;
}

/// Orders services, responsible for Creating orders
pub struct OrderServiceImpl {
    pub http_client: Arc<HttpClientHandle>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateOrderOperationLog>>,
    pub user_id: Option<UserId>,
}

impl OrderServiceImpl {
    pub fn new(http_client: Arc<HttpClientHandle>, config: config::Config, user_id: Option<UserId>) -> Self {
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

        let orders_url = self.config.service_url(StqService::Orders);
        let rpc_client = RestApiClient::new(&orders_url, self.user_id.clone());
        let res = rpc_client
            .convert_cart(
                Some(convert_cart.conversion_id),
                convert_cart.convert_cart.customer_id,
                convert_cart.convert_cart.prices,
                convert_cart.convert_cart.address,
                convert_cart.convert_cart.receiver_name,
                convert_cart.convert_cart.receiver_phone,
            )
            .map_err(|e| {
                e.context("Converting cart in orders microservice failed.")
                    .context(Error::RpcClient)
                    .into()
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateOrderOperationStage::OrdersConvertCartComplete(convertion_id));
            })
            .then(|res| match res {
                Ok(orders) => Ok((self, orders)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_invoice(self, input: &CreateInvoice) -> ServiceFuture<Self, Invoice> {
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
                        e.context("Creating invoice in billing microservice failed.")
                            .context(Error::HttpClient)
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

    fn notify_user_create_order(&self, user_id: UserId, order_slug: OrderSlug) -> Box<Future<Item = (), Error = FailureError>> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let users_url = self.config.service_url(StqService::Users);
        let cluster_url = self.config.cluster.url.clone();
        let url = format!("{}/{}/{}", users_url, StqModel::User.to_url(), user_id);
        let mut headers = Headers::new();
        headers.set(Authorization(user_id.to_string()));
        let send_to_client = client
            .request::<Option<User>>(Method::Get, url, None, Some(headers))
            .map_err(From::from)
            .and_then({
                let client = client.clone();
                move |user| {
                    if let Some(user) = user {
                        let user = EmailUser {
                            email: user.email.clone(),
                            first_name: user.first_name.unwrap_or("user".to_string()),
                            last_name: user.last_name.unwrap_or("".to_string()),
                        };
                        let email = OrderCreateForUser {
                            user,
                            order_slug: order_slug.to_string(),
                            cluster_url,
                        };
                        let url = format!("{}/users/order-create", notifications_url);
                        Box::new(
                            serde_json::to_string(&email)
                                .map_err(From::from)
                                .into_future()
                                .and_then(move |body| client.request::<()>(Method::Post, url, Some(body), None).map_err(From::from)),
                        ) as Box<Future<Item = (), Error = FailureError>>
                    } else {
                        error!(
                            "Sending notification to user can not be done. User with id: {} is not found.",
                            user_id
                        );
                        Box::new(future::err(
                            format_err!("User is not found in users microservice.")
                                .context(Error::NotFound)
                                .into(),
                        ))
                    }
                }
            });

        Box::new(send_to_client)
    }

    fn notify_store_create_order(&self, store_id: StoreId, order_slug: OrderSlug) -> Box<Future<Item = (), Error = FailureError>> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let stores_url = self.config.service_url(StqService::Stores);
        let cluster_url = self.config.cluster.url.clone();
        let url = format!("{}/{}/{}", stores_url, StqModel::Store.to_url(), store_id);
        let send_to_store = client
            .request::<Option<Store>>(Method::Get, url, None, None)
            .map_err(From::from)
            .and_then({
                let client = client.clone();
                let notifications_url = notifications_url.clone();
                let cluster_url = cluster_url.clone();
                move |store| {
                    if let Some(store) = store {
                        if let Some(store_email) = store.email {
                            let email = OrderCreateForStore {
                                store_email,
                                store_id: store_id.to_string(),
                                order_slug: order_slug.to_string(),
                                cluster_url,
                            };
                            let url = format!("{}/stores/order-create", notifications_url);
                            Box::new(
                                serde_json::to_string(&email)
                                    .map_err(From::from)
                                    .into_future()
                                    .and_then(move |body| client.request::<()>(Method::Post, url, Some(body), None).map_err(From::from)),
                            ) as Box<Future<Item = (), Error = FailureError>>
                        } else {
                            Box::new(future::ok(()))
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
            });

        Box::new(send_to_store)
    }

    fn notify_user_update_order(
        &self,
        user_id: UserId,
        order_slug: OrderSlug,
        order_state: String,
    ) -> Box<Future<Item = (), Error = FailureError>> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let users_url = self.config.service_url(StqService::Users);
        let cluster_url = self.config.cluster.url.clone();
        let url = format!("{}/{}/{}", users_url, StqModel::User.to_url(), user_id);
        let mut headers = Headers::new();
        headers.set(Authorization(user_id.to_string()));
        let send_to_client = client
            .request::<Option<User>>(Method::Get, url, None, Some(headers))
            .map_err(From::from)
            .and_then({
                let client = client.clone();
                move |user| {
                    if let Some(user) = user {
                        let user = EmailUser {
                            email: user.email.clone(),
                            first_name: user.first_name.unwrap_or("user".to_string()),
                            last_name: user.last_name.unwrap_or("".to_string()),
                        };
                        let email = OrderUpdateStateForUser {
                            user,
                            order_slug: order_slug.to_string(),
                            order_state,
                            cluster_url,
                        };
                        let url = format!("{}/users/order-update-state", notifications_url);
                        Box::new(
                            serde_json::to_string(&email)
                                .map_err(From::from)
                                .into_future()
                                .and_then(move |body| client.request::<()>(Method::Post, url, Some(body), None).map_err(From::from)),
                        ) as Box<Future<Item = (), Error = FailureError>>
                    } else {
                        error!(
                            "Sending notification to user can not be done. User with id: {} is not found.",
                            user_id
                        );
                        Box::new(future::err(
                            format_err!("User is not found in users microservice.")
                                .context(Error::NotFound)
                                .into(),
                        ))
                    }
                }
            });

        Box::new(send_to_client)
    }

    fn notify_store_update_order(
        &self,
        store_id: StoreId,
        order_slug: OrderSlug,
        order_state: String,
    ) -> Box<Future<Item = (), Error = FailureError>> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let stores_url = self.config.service_url(StqService::Stores);
        let cluster_url = self.config.cluster.url.clone();
        let url = format!("{}/{}/{}", stores_url, StqModel::Store.to_url(), store_id);
        let send_to_store = client
            .request::<Option<Store>>(Method::Get, url, None, None)
            .map_err(From::from)
            .and_then({
                let client = client.clone();
                let notifications_url = notifications_url.clone();
                let cluster_url = cluster_url.clone();
                move |store| {
                    if let Some(store) = store {
                        if let Some(store_email) = store.email {
                            let email = OrderUpdateStateForStore {
                                store_email,
                                store_id: store.id.to_string(),
                                order_slug: order_slug.to_string(),
                                order_state,
                                cluster_url,
                            };
                            let url = format!("{}/stores/order-update-state", notifications_url);
                            Box::new(
                                serde_json::to_string(&email)
                                    .map_err(From::from)
                                    .into_future()
                                    .and_then(move |body| client.request::<()>(Method::Post, url, Some(body), None).map_err(From::from)),
                            ) as Box<Future<Item = (), Error = FailureError>>
                        } else {
                            Box::new(future::ok(()))
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
            });

        Box::new(send_to_store)
    }

    fn notify_on_create(self, orders: &[Order]) -> ServiceFuture<Self, ()> {
        let mut orders_futures = vec![];
        for order in orders {
            let send_to_client = self.notify_user_create_order(order.customer, order.slug);
            let send_to_store = self.notify_store_create_order(order.store, order.slug);
            let res = Box::new(send_to_client.then(|_| send_to_store).then(|_| Ok(())));
            orders_futures.push(Box::new(res) as Box<Future<Item = (), Error = FailureError>>);
        }

        Box::new(
            join_all(orders_futures)
                .map_err(|e: FailureError| e.context("Notifying on create orders error.".to_string()).into())
                .then(|res| match res {
                    Ok(_) => Ok((self, ())),
                    Err(e) => Err((self, e)),
                }),
        )
    }

    fn notify_on_update(self, orders: &[Option<Order>]) -> ServiceFuture<Self, ()> {
        let mut orders_futures = vec![];
        for order in orders {
            if let Some(order) = order {
                let send_to_client = self.notify_user_update_order(order.customer, order.slug, order.state.to_string());
                let send_to_store = self.notify_store_update_order(order.store, order.slug, order.state.to_string());
                let res = Box::new(send_to_client.then(|_| send_to_store).then(|_| Ok(())));
                orders_futures.push(Box::new(res) as Box<Future<Item = (), Error = FailureError>>);
            }
        }

        Box::new(
            join_all(orders_futures)
                .map_err(|e: FailureError| e.context("Notifying on update orders error.".to_string()).into())
                .then(|res| match res {
                    Ok(_) => Ok((self, ())),
                    Err(e) => Err((self, e)),
                }),
        )
    }

    // Contains happy path for Order creation
    fn create_happy(self, input: ConvertCart) -> ServiceFuture<Self, Invoice> {
        Box::new(self.convert_cart(input.clone()).and_then(move |(s, orders)| {
            let orders = orders.clone();
            let create_invoice = CreateInvoice {
                customer_id: input.customer_id,
                orders: orders.clone(),
                currency_id: input.currency_id,
                saga_id: SagaId::new(),
            };
            s.create_invoice(&create_invoice).and_then(move |(s, invoice)| {
                s.notify_on_create(&orders).then(|res| match res {
                    Ok((s, _)) => Ok((s, invoice)),
                    Err((s, _)) => Ok((s, invoice)),
                })
            })
        }))
    }

    // Contains happy path for Order creation
    fn update_orders_happy(self, orders_info: BillingOrdersVec) -> ServiceFuture<Self, ()> {
        Box::new(self.update_orders(orders_info).and_then(move |(s, orders)| {
            s.notify_on_update(&orders).then(|res| match res {
                Ok((s, _)) => Ok((s, ())),
                Err((s, _)) => Ok((s, ())),
            })
        }))
    }

    fn update_orders(self, orders_info: BillingOrdersVec) -> ServiceFuture<Self, Vec<Option<Order>>> {
        debug!("Updating orders status: {}", orders_info);

        let client = self.http_client.clone();
        let orders_url = self.config.service_url(StqService::Orders);

        let mut orders_futures = vec![];
        for order_info in orders_info.0 {
            match &order_info.status {
                OrderState::AmountExpired | OrderState::TransactionPending => continue, // do not set these invoice statuses to orders
                _ => {}
            }
            let payload: UpdateStatePayload = order_info.clone().into();
            let body = serde_json::to_string(&payload).unwrap_or_default();
            let url = format!("{}/{}/by-id/{}", orders_url.clone(), StqModel::Order.to_url(), order_info.order_id);
            let mut headers = Headers::new();
            headers.set(Authorization(order_info.customer_id.0.to_string()));
            let res = client
                .request::<Option<Order>>(Method::Get, url, None, Some(headers))
                .map_err(|e| {
                    e.context("Setting new status in orders microservice error occured.")
                        .context(Error::HttpClient)
                        .into()
                })
                .and_then({
                    let client = client.clone();
                    let orders_url = orders_url.clone();
                    move |order| {
                        if let Some(order) = order {
                            if order.state == order_info.status {
                                // if this status already set, do not update
                                Box::new(future::ok(None)) as Box<Future<Item = Option<Order>, Error = FailureError>>
                            } else {
                                let url = format!("{}/{}/by-id/{}/status", orders_url, StqModel::Order.to_url(), order.id);
                                let mut headers = Headers::new();
                                headers.set(Authorization("1".to_string()));
                                Box::new(
                                    client
                                        .request::<Option<Order>>(Method::Put, url, Some(body.clone()), Some(headers))
                                        .map_err(|e| {
                                            e.context("Setting new status in orders microservice error occured.")
                                                .context(Error::HttpClient)
                                                .into()
                                        }),
                                ) as Box<Future<Item = Option<Order>, Error = FailureError>>
                            }
                        } else {
                            Box::new(future::err(
                                format_err!("Order is not found in orders microservice! id: {}", order_info.order_id.0)
                                    .context(Error::NotFound)
                                    .into(),
                            )) as Box<Future<Item = Option<Order>, Error = FailureError>>
                        }
                    }
                });
            orders_futures.push(Box::new(res) as Box<Future<Item = Option<Order>, Error = FailureError>>);
        }

        Box::new(join_all(orders_futures).then(|res| match res {
            Ok(orders) => Ok((self, orders)),
            Err(e) => Err((self, e)),
        }))
    }

    // Contains reversal of Order creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();

        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log {
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
                                    e.context("Order service create_revert OrdersConvertCartStart error occured.")
                                        .context(Error::HttpClient)
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
                                format!("{}/invoices/by-saga-id/{}", s.config.service_url(StqService::Billing), saga_id.0,),
                                None,
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    e.context("Order service create_revert BillingCreateInvoiceStart error occured.")
                                        .context(Error::HttpClient)
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
                        futures::future::err((Box::new(s) as Box<OrderService>, e))
                    })
                })
                .map_err(|(s, e): (Box<OrderService>, FailureError)| (s, parse_validation_errors(e, &["phone"]))),
        )
    }

    fn update_state(self, orders_info: BillingOrdersVec) -> ServiceFuture<Box<OrderService>, ()> {
        debug!("Updating orders status: {}", orders_info);
        Box::new(
            self.update_orders_happy(orders_info)
                .map(|(s, _)| (Box::new(s) as Box<OrderService>, ()))
                .or_else(|(s, e)| futures::future::err((Box::new(s) as Box<OrderService>, e))),
        )
    }
}

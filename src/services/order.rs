use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures::future::{self, join_all, Either};
use futures::prelude::*;
use futures::stream::iter_ok;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;
use serde_json;

use stq_api::orders::{BuyNow, Order};
use stq_api::rpc_client::RestApiClient;
use stq_api::warehouses::WarehouseClient;
use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_static_resources::{
    EmailUser, OrderCreateForStore, OrderCreateForUser, OrderState, OrderUpdateStateForStore, OrderUpdateStateForUser,
};
use stq_types::{ConversionId, CouponId, OrderIdentifier, OrderSlug, Quantity, SagaId, StoreId, UserId};

use super::parse_validation_errors;
use config;
use errors::Error;
use microservice::{OrdersMicroservice, StoresMicroservice};
use models::*;
use services::types::ServiceFuture;

pub trait OrderService {
    fn create(self, input: ConvertCart) -> ServiceFuture<Box<OrderService>, Invoice>;
    fn create_buy_now(self, input: BuyNow) -> ServiceFuture<Box<OrderService>, Invoice>;
    fn update_state_by_billing(self, orders_info: BillingOrdersVec) -> ServiceFuture<Box<OrderService>, ()>;
    fn manual_set_state(
        self,
        order_slug: OrderSlug,
        order_state: OrderState,
        track_id: Option<String>,
        comment: Option<String>,
    ) -> ServiceFuture<Box<OrderService>, Option<Order>>;
}

/// Orders services, responsible for Creating orders
pub struct OrderServiceImpl {
    pub http_client: HttpClientHandle,
    pub orders_microservice: Box<OrdersMicroservice>,
    pub stores_microservice: Box<StoresMicroservice>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateOrderOperationLog>>,
    pub user_id: Option<UserId>,
}

impl OrderServiceImpl {
    pub fn new(
        http_client: HttpClientHandle,
        config: config::Config,
        user_id: Option<UserId>,
        orders_microservice: Box<OrdersMicroservice>,
        stores_microservice: Box<StoresMicroservice>,
    ) -> Self {
        let log = Arc::new(Mutex::new(CreateOrderOperationLog::new()));
        Self {
            http_client,
            config,
            log,
            user_id,
            orders_microservice,
            stores_microservice,
        }
    }

    fn convert_cart(self, input: ConvertCart) -> impl Future<Item = (Self, Vec<Order>), Error = (Self, FailureError)> {
        // Create Order
        debug!("Converting cart, input: {:?}", input);
        let convert_cart: ConvertCartWithConversionId = input.into();
        let convertion_id = convert_cart.conversion_id;
        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateOrderOperationStage::OrdersConvertCartStart(convertion_id));

        self.orders_microservice
            .convert_cart(
                Some(convert_cart.conversion_id),
                convert_cart.convert_cart.customer_id,
                convert_cart.convert_cart.prices,
                convert_cart.convert_cart.address,
                convert_cart.convert_cart.receiver_name,
                convert_cart.convert_cart.receiver_phone,
                convert_cart.convert_cart.receiver_email,
                convert_cart.convert_cart.coupons,
                convert_cart.convert_cart.delivery_info,
            ).map_err(|e| {
                parse_validation_errors(e.into(), &["order"])
                    .context("Converting cart in orders microservice failed.")
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateOrderOperationStage::OrdersConvertCartComplete(convertion_id));
                Ok(res)
            }).then(|res| match res {
                Ok(orders) => Ok((self, orders)),
                Err(e) => Err((self, e)),
            })
    }

    fn commit_coupon(self, payload: (CouponId, UserId)) -> impl Future<Item = (Self, UsedCoupon), Error = (Self, FailureError)> {
        let (coupon_id, customer) = payload;

        self.stores_microservice
            .with_superadmin()
            .use_coupon(coupon_id, customer)
            .map_err(|e| {
                e.context("Commit coupon for user in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).then(|res| match res {
                Ok(used_coupon) => Ok((self, used_coupon)),
                Err(e) => Err((self, e)),
            })
    }

    fn commit_coupons(self, input: Vec<Order>) -> impl Future<Item = (Self, Vec<UsedCoupon>), Error = (Self, FailureError)> {
        debug!("Commit coupons");

        let mut payload = vec![];
        for order in input {
            if let Some(coupon_id) = order.coupon_id {
                payload.push((coupon_id, order.customer));
            }
        }

        let payload = payload.into_iter().collect::<HashMap<CouponId, UserId>>();

        let fut = iter_ok::<_, (Self, FailureError)>(payload).fold((self, vec![]), move |(s, mut used_coupons), order| {
            s.commit_coupon(order).and_then(|(s, res)| {
                used_coupons.push(res);

                Ok((s, used_coupons)) as Result<(Self, Vec<UsedCoupon>), (Self, FailureError)>
            })
        });

        fut
    }

    fn buy_now(self, input: BuyNow) -> impl Future<Item = (Self, Vec<Order>), Error = (Self, FailureError)> {
        // Create Order
        debug!("Create order from buy_now input: {:?}", input);
        let conversion_id = ConversionId::new();

        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateOrderOperationStage::OrdersConvertCartStart(conversion_id));

        self.orders_microservice
            .create_buy_now(input, Some(conversion_id))
            .map_err(|e| {
                parse_validation_errors(e.into(), &["order"])
                    .context("Create order from buy now data in orders microservice failed.")
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateOrderOperationStage::OrdersConvertCartComplete(conversion_id));
                Ok(res)
            }).then(|res| match res {
                Ok(orders) => Ok((self, orders)),
                Err(e) => Err((self, e)),
            })
    }

    fn create_invoice(self, input: &CreateInvoice) -> impl Future<Item = (Self, Invoice), Error = (Self, FailureError)> {
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

        serde_json::to_string(&input)
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
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateOrderOperationStage::BillingCreateInvoiceComplete(saga_id));
                Ok(res)
            }).then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((self, e)),
            })
    }

    fn notify_user_create_order(&self, user_id: UserId, order_slug: OrderSlug) -> impl Future<Item = (), Error = FailureError> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let users_url = self.config.service_url(StqService::Users);
        let cluster_url = self.config.cluster.url.clone();
        let url = format!("{}/{}/{}", users_url, StqModel::User.to_url(), user_id);
        let mut headers = Headers::new();
        headers.set(Authorization(user_id.to_string()));
        client
            .request::<Option<User>>(Method::Get, url, None, Some(headers))
            .map_err(From::from)
            .and_then({
                let client = client.clone();
                move |user| {
                    if let Some(user) = user {
                        let user = EmailUser {
                            email: user.email.clone(),
                            first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                            last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                        };
                        let email = OrderCreateForUser {
                            user,
                            order_slug: order_slug.to_string(),
                            cluster_url,
                        };
                        let url = format!("{}/users/order-create", notifications_url);
                        Either::A(
                            serde_json::to_string(&email)
                                .map_err(From::from)
                                .into_future()
                                .and_then(move |body| {
                                    let mut headers = Headers::new();
                                    headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                    client
                                        .request::<()>(Method::Post, url, Some(body), Some(headers))
                                        .map_err(From::from)
                                }),
                        )
                    } else {
                        error!(
                            "Sending notification to user can not be done. User with id: {} is not found.",
                            user_id
                        );
                        Either::B(future::err(
                            format_err!("User is not found in users microservice.")
                                .context(Error::NotFound)
                                .into(),
                        ))
                    }
                }
            })
    }

    fn notify_store_create_order(&self, store_id: StoreId, order_slug: OrderSlug) -> impl Future<Item = (), Error = FailureError> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let cluster_url = self.config.cluster.url.clone();
        self.stores_microservice.get(store_id).and_then({
            let client = client.clone();
            let notifications_url = notifications_url.clone();
            let cluster_url = cluster_url.clone();
            move |store| {
                if let Some(store) = store {
                    Either::A(if let Some(store_email) = store.email {
                        let email = OrderCreateForStore {
                            store_email,
                            store_id: store_id.to_string(),
                            order_slug: order_slug.to_string(),
                            cluster_url,
                        };
                        let url = format!("{}/stores/order-create", notifications_url);
                        Either::A(
                            serde_json::to_string(&email)
                                .map_err(From::from)
                                .into_future()
                                .and_then(move |body| {
                                    let mut headers = Headers::new();
                                    headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                    client
                                        .request::<()>(Method::Post, url, Some(body), Some(headers))
                                        .map_err(From::from)
                                }),
                        )
                    } else {
                        Either::B(future::ok(()))
                    })
                } else {
                    error!(
                        "Sending notification to store can not be done. Store with id: {} is not found.",
                        store_id
                    );
                    Either::B(future::err(
                        format_err!("Store is not found in stores microservice.")
                            .context(Error::NotFound)
                            .into(),
                    ))
                }
            }
        })
    }

    fn notify_user_update_order(
        &self,
        user_id: UserId,
        order_slug: OrderSlug,
        order_state: OrderState,
    ) -> impl Future<Item = (), Error = FailureError> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let users_url = self.config.service_url(StqService::Users);
        let cluster_url = self.config.cluster.url.clone();
        let url = format!("{}/{}/{}", users_url, StqModel::User.to_url(), user_id);
        let mut headers = Headers::new();
        headers.set(Authorization(user_id.to_string()));
        client
            .request::<Option<User>>(Method::Get, url, None, Some(headers))
            .map_err(From::from)
            .and_then({
                let client = client.clone();
                move |user| {
                    if let Some(user) = user {
                        let user = EmailUser {
                            email: user.email.clone(),
                            first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                            last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                        };
                        let email = OrderUpdateStateForUser {
                            user,
                            order_slug: order_slug.to_string(),
                            order_state: order_state.to_string(),
                            cluster_url,
                        };
                        let url = format!("{}/users/order-update-state", notifications_url);
                        Either::A(
                            serde_json::to_string(&email)
                                .map_err(From::from)
                                .into_future()
                                .and_then(move |body| {
                                    let mut headers = Headers::new();
                                    headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                    client
                                        .request::<()>(Method::Post, url, Some(body), Some(headers))
                                        .map_err(From::from)
                                }),
                        )
                    } else {
                        error!(
                            "Sending notification to user can not be done. User with id: {} is not found.",
                            user_id
                        );
                        Either::B(future::err(
                            format_err!("User is not found in users microservice.")
                                .context(Error::NotFound)
                                .into(),
                        ))
                    }
                }
            })
    }

    fn notify_store_update_order(
        &self,
        store_id: StoreId,
        order_slug: OrderSlug,
        order_state: OrderState,
    ) -> impl Future<Item = (), Error = FailureError> {
        let client = self.http_client.clone();
        let notifications_url = self.config.service_url(StqService::Notifications);
        let cluster_url = self.config.cluster.url.clone();
        self.stores_microservice.get(store_id).and_then({
            let client = client.clone();
            let notifications_url = notifications_url.clone();
            let cluster_url = cluster_url.clone();
            move |store| {
                if let Some(store) = store {
                    Either::A(if let Some(store_email) = store.email {
                        let email = OrderUpdateStateForStore {
                            store_email,
                            store_id: store.id.to_string(),
                            order_slug: order_slug.to_string(),
                            order_state: order_state.to_string(),
                            cluster_url,
                        };
                        let url = format!("{}/stores/order-update-state", notifications_url);
                        Either::A(
                            serde_json::to_string(&email)
                                .map_err(From::from)
                                .into_future()
                                .and_then(move |body| {
                                    let mut headers = Headers::new();
                                    headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                    client
                                        .request::<()>(Method::Post, url, Some(body), Some(headers))
                                        .map_err(From::from)
                                }),
                        )
                    } else {
                        Either::B(future::ok(()))
                    })
                } else {
                    error!(
                        "Sending notification to store can not be done. Store with id: {} is not found.",
                        store_id
                    );
                    Either::B(future::err(
                        format_err!("Store is not found in stores microservice.")
                            .context(Error::NotFound)
                            .into(),
                    ))
                }
            }
        })
    }

    fn notify(self, orders: &[Option<Order>]) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let mut orders_futures = vec![];
        for order in orders {
            if let Some(order) = order {
                let send_to_client = match order.state {
                    OrderState::New | OrderState::PaymentAwaited | OrderState::TransactionPending | OrderState::AmountExpired => {
                        Box::new(future::ok(())) as Box<Future<Item = (), Error = FailureError>>
                    }
                    OrderState::Paid => {
                        Box::new(self.notify_user_create_order(order.customer, order.slug)) as Box<Future<Item = (), Error = FailureError>>
                    }
                    OrderState::InProcessing
                    | OrderState::Cancelled
                    | OrderState::Sent
                    | OrderState::Delivered
                    | OrderState::Received
                    | OrderState::Complete => Box::new(self.notify_user_update_order(order.customer, order.slug, order.state))
                        as Box<Future<Item = (), Error = FailureError>>,
                };
                let send_to_store = match order.state {
                    OrderState::New | OrderState::PaymentAwaited | OrderState::TransactionPending | OrderState::AmountExpired => {
                        Box::new(future::ok(())) as Box<Future<Item = (), Error = FailureError>>
                    }
                    OrderState::Paid => {
                        Box::new(self.notify_store_create_order(order.store, order.slug)) as Box<Future<Item = (), Error = FailureError>>
                    }
                    OrderState::InProcessing
                    | OrderState::Cancelled
                    | OrderState::Sent
                    | OrderState::Delivered
                    | OrderState::Received
                    | OrderState::Complete => Box::new(self.notify_store_update_order(order.store, order.slug, order.state))
                        as Box<Future<Item = (), Error = FailureError>>,
                };

                let res = send_to_client.then(|_| send_to_store).then(|_| Ok(()));
                orders_futures.push(res);
            }
        }

        join_all(orders_futures)
            .map_err(|e: FailureError| e.context("Notifying on update orders error.".to_string()).into())
            .then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(e) => Err((self, e)),
            })
    }

    // Contains happy path for Order creation
    fn create_happy(self, input: ConvertCart) -> impl Future<Item = (Self, Invoice), Error = (Self, FailureError)> {
        self.convert_cart(input.clone()).and_then(move |(s, orders)| {
            let create_invoice = CreateInvoice {
                customer_id: input.customer_id,
                orders: orders.clone(),
                currency: input.currency,
                saga_id: SagaId::new(),
            };
            s.create_invoice(&create_invoice).and_then(move |(s, invoice)| {
                s.commit_coupons(orders.clone()).and_then(move |(s, _)| {
                    s.notify(&orders.into_iter().map(Some).collect::<Vec<Option<Order>>>())
                        .then(|res| match res {
                            Ok((s, _)) => Ok((s, invoice)),
                            Err((s, _)) => Ok((s, invoice)),
                        })
                })
            })
        })
    }

    fn create_from_buy_now(self, input: BuyNow) -> impl Future<Item = (Self, Invoice), Error = (Self, FailureError)> {
        self.buy_now(input.clone()).and_then(move |(s, orders)| {
            let create_invoice = CreateInvoice {
                customer_id: input.customer_id,
                orders: orders.clone(),
                currency: input.currency,
                saga_id: SagaId::new(),
            };
            s.create_invoice(&create_invoice).and_then(move |(s, invoice)| {
                s.notify(&orders.into_iter().map(Some).collect::<Vec<Option<Order>>>())
                    .then(|res| match res {
                        Ok((s, _)) => Ok((s, invoice)),
                        Err((s, _)) => Ok((s, invoice)),
                    })
            })
        })
    }

    // Contains happy path for Order creation
    fn update_orders_happy(self, orders_info: BillingOrdersVec) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        self.update_orders(orders_info)
            .and_then(move |(s, orders)| {
                s.update_warehouse(&orders).then(|res| match res {
                    Ok((s, _)) => Ok((s, orders)),
                    Err((s, _)) => Ok((s, orders)),
                })
            }).and_then(move |(s, orders)| {
                s.notify(&orders).then(|res| match res {
                    Ok((s, _)) => Ok((s, ())),
                    Err((s, _)) => Ok((s, ())),
                })
            })
    }

    // Contains happy path for Order set state
    fn set_state_happy(
        self,
        order_slug: OrderSlug,
        order_state: OrderState,
        track_id: Option<String>,
        comment: Option<String>,
    ) -> impl Future<Item = (Self, Option<Order>), Error = (Self, FailureError)> {
        self.set_state(order_slug, order_state, track_id, comment)
            .and_then(move |(s, order)| {
                s.notify(&[order.clone()]).then(|res| match res {
                    Ok((s, _)) => Ok((s, order)),
                    Err((s, _)) => Ok((s, order)),
                })
            })
    }

    fn update_orders(self, orders_info: BillingOrdersVec) -> impl Future<Item = (Self, Vec<Option<Order>>), Error = (Self, FailureError)> {
        debug!("Updating orders status: {}", orders_info);

        let mut orders_futures = vec![];
        for order_info in orders_info.0 {
            match &order_info.status {
                OrderState::AmountExpired | OrderState::TransactionPending => continue, // do not set these invoice statuses to orders
                _ => {}
            }

            let orders_microservice = self.orders_microservice.cloned();

            let order_id = order_info.order_id;

            let res = self
                .orders_microservice
                .with_user(order_info.customer_id)
                .get_order(OrderIdentifier::Id(order_info.order_id))
                .map_err(|e| {
                    e.context("Setting new status in orders microservice error occured.")
                        .context(Error::HttpClient)
                        .into()
                }).and_then(move |order| {
                    order
                        .ok_or(
                            format_err!("Order is not found in orders microservice! id: {}", order_id)
                                .context(Error::NotFound)
                                .into(),
                        ).into_future()
                }).and_then(move |order| {
                    if order.state == order_info.status {
                        // if this status already set, do not update
                        Either::A(future::ok(None))
                    } else {
                        let payload: UpdateStatePayload = order_info.clone().into();
                        Either::B(
                            orders_microservice
                                .with_superadmin()
                                .set_order_state(OrderIdentifier::Id(order.id), payload)
                                .map_err(|e| {
                                    e.context("Setting new status in orders microservice error occured.")
                                        .context(Error::HttpClient)
                                        .into()
                                }),
                        )
                    }
                });
            orders_futures.push(res);
        }

        join_all(orders_futures).then(|res| match res {
            Ok(orders) => Ok((self, orders)),
            Err(e) => Err((self, e)),
        })
    }

    fn set_state(
        self,
        order_slug: OrderSlug,
        order_state: OrderState,
        track_id: Option<String>,
        comment: Option<String>,
    ) -> impl Future<Item = (Self, Option<Order>), Error = (Self, FailureError)> {
        let orders_microservice = self.orders_microservice.cloned();
        self.orders_microservice
            .get_order(OrderIdentifier::Slug(order_slug))
            .map_err(move |e| {
                parse_validation_errors(e.into(), &["order"])
                    .context(format!("Getting order with slug {} in orders microservice failed.", order_slug))
                    .into()
            }).and_then(move |order| {
                if let Some(order) = order {
                    Either::A(if order.state == order_state {
                        // if this status already set, do not update
                        info!("order slug: {:?} status: {:?} already set, do not update", order_slug, order_state);
                        Either::A(future::ok(None))
                    } else {
                        info!(
                            "order slug: {:?} status: {:?} start request update on orders",
                            order_slug, order_state
                        );
                        Either::B(
                            orders_microservice
                                .set_order_state(
                                    OrderIdentifier::Slug(order_slug),
                                    UpdateStatePayload {
                                        state: order_state,
                                        comment,
                                        track_id,
                                    },
                                ).map_err(move |e| {
                                    parse_validation_errors(e.into(), &["order"])
                                        .context(format!(
                                            "Setting order with slug {} state {} in orders microservice failed.",
                                            order_slug, order_state
                                        )).into()
                                }),
                        )
                    })
                } else {
                    Either::B(future::err(
                        format_err!("Order is not found in orders microservice! slug: {}", order_slug)
                            .context(Error::NotFound)
                            .into(),
                    ))
                }
            }).then(|res| match res {
                Ok(order) => Ok((self, order)),
                Err(e) => Err((self, e)),
            })
    }

    fn update_warehouse(self, orders: &[Option<Order>]) -> impl Future<Item = (Self, Vec<()>), Error = (Self, FailureError)> {
        debug!("Updating warehouses stock: {:?}", orders);

        let warehouses_url = self.config.service_url(StqService::Warehouses);
        let mut orders_futures = vec![];
        for order in orders {
            if let Some(order) = order {
                if order.state == OrderState::Paid {
                    debug!("Updating warehouses stock with product id {}", order.product);
                    let rpc_client = RestApiClient::new(&warehouses_url, Some(UserId(1))); // sending update from super user
                    let order_quantity = order.quantity;
                    let res = rpc_client
                        .find_by_product_id(order.product)
                        .and_then(move |stocks| {
                            debug!("Updating warehouses stocks: {:?}", stocks);
                            for stock in stocks {
                                let new_quantity = if stock.quantity.0 > order_quantity.0 {
                                    stock.quantity.0 - order_quantity.0
                                } else {
                                    0
                                };
                                debug!(
                                    "New warehouses {} product {} quantity {}",
                                    stock.warehouse_id, stock.product_id, new_quantity
                                );
                                return Either::A(
                                    rpc_client
                                        .set_product_in_warehouse(stock.warehouse_id, stock.product_id, Quantity(new_quantity))
                                        .map(|_| ()),
                                );
                            }
                            Either::B(future::ok(()))
                        }).map_err(|e| {
                            let err = e
                                .context("decrementing quantity in warehouses microservice failed.")
                                .context(Error::RpcClient)
                                .into();
                            error!("{}", err);
                            err
                        });

                    orders_futures.push(res);
                }
            }
        }

        join_all(orders_futures).then(|res| match res {
            Ok(orders) => Ok((self, orders)),
            Err(e) => Err((self, e)),
        })
    }

    // Contains reversal of Order creation
    fn create_revert(self) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let log = self.log.lock().unwrap().clone();
        let http_client = self.http_client.clone();
        let orders_microservice = self.orders_microservice.cloned();
        let billing_url = self.config.service_url(StqService::Billing);

        let fut = iter_ok::<_, ()>(log).for_each(move |e| {
            match e {
                CreateOrderOperationStage::OrdersConvertCartComplete(conversion_id) => {
                    debug!("Reverting cart convertion, conversion_id: {}", conversion_id);
                    let result = orders_microservice
                        .with_superadmin()
                        .revert_convert_cart(ConvertCartRevert { conversion_id });

                    Box::new(result.then(|_| Ok(()))) as Box<Future<Item = (), Error = ()>>
                }

                CreateOrderOperationStage::BillingCreateInvoiceComplete(saga_id) => {
                    debug!("Reverting create invoice, saga_id: {}", saga_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin can revert invoice

                    Box::new(
                        http_client
                            .request::<SagaId>(
                                Method::Delete,
                                format!("{}/invoices/by-saga-id/{}", billing_url, saga_id.0,),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                _ => Box::new(future::ok(())) as Box<Future<Item = (), Error = ()>>,
            }
        });

        fut.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(_) => Err((self, format_err!("Order service create_revert error occured."))),
        })
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
                        future::err((Box::new(s) as Box<OrderService>, e))
                    })
                }).map_err(|(s, e): (Box<OrderService>, FailureError)| (s, parse_validation_errors(e, &["phone"]))),
        )
    }

    fn create_buy_now(self, input: BuyNow) -> ServiceFuture<Box<OrderService>, Invoice> {
        Box::new(
            self.create_from_buy_now(input)
                .map(|(s, order)| (Box::new(s) as Box<OrderService>, order))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<OrderService>, e)))
                .map_err(|(s, e): (Box<OrderService>, FailureError)| (s, parse_validation_errors(e, &["phone"]))),
        )
    }

    fn update_state_by_billing(self, orders_info: BillingOrdersVec) -> ServiceFuture<Box<OrderService>, ()> {
        debug!("Updating orders status: {}", orders_info);
        Box::new(
            self.update_orders_happy(orders_info)
                .map(|(s, _)| (Box::new(s) as Box<OrderService>, ()))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<OrderService>, e))),
        )
    }

    fn manual_set_state(
        self,
        order_slug: OrderSlug,
        order_state: OrderState,
        track_id: Option<String>,
        comment: Option<String>,
    ) -> ServiceFuture<Box<OrderService>, Option<Order>> {
        debug!(
            "set order {} status '{}' with track {:?} and comment {:?}",
            order_slug, order_state, track_id, comment
        );
        Box::new(
            self.set_state_happy(order_slug, order_state, track_id, comment)
                .map(|(s, o)| (Box::new(s) as Box<OrderService>, o))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<OrderService>, e))),
        )
    }
}

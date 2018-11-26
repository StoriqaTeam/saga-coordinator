use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures::future::{self, join_all, Either};
use futures::prelude::*;
use futures::stream::iter_ok;

use stq_api::orders::Order;
use stq_static_resources::{
    EmailUser, OrderCreateForStore, OrderCreateForUser, OrderState, OrderUpdateStateForStore, OrderUpdateStateForUser,
};
use stq_types::{ConversionId, CouponId, OrderIdentifier, OrderSlug, Quantity, SagaId, StoreId, UserId};

use super::parse_validation_errors;
use config;
use errors::Error;
use microservice::{
    BillingMicroservice, Initiator, NotificationsMicroservice, OrdersMicroservice, StoresMicroservice, UsersMicroservice,
    WarehousesMicroservice,
};
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
    pub orders_microservice: Arc<OrdersMicroservice>,
    pub stores_microservice: Arc<StoresMicroservice>,
    pub notifications_microservice: Arc<NotificationsMicroservice>,
    pub users_microservice: Arc<UsersMicroservice>,
    pub billing_microservice: Arc<BillingMicroservice>,
    pub warehouses_microservice: Arc<WarehousesMicroservice>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateOrderOperationLog>>,
}

impl OrderServiceImpl {
    pub fn new(
        config: config::Config,
        orders_microservice: Arc<OrdersMicroservice>,
        stores_microservice: Arc<StoresMicroservice>,
        notifications_microservice: Arc<NotificationsMicroservice>,
        users_microservice: Arc<UsersMicroservice>,
        billing_microservice: Arc<BillingMicroservice>,
        warehouses_microservice: Arc<WarehousesMicroservice>,
    ) -> Self {
        let log = Arc::new(Mutex::new(CreateOrderOperationLog::new()));
        Self {
            config,
            log,
            orders_microservice,
            stores_microservice,
            notifications_microservice,
            users_microservice,
            billing_microservice,
            warehouses_microservice,
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
            .convert_cart(ConvertCartPayload {
                conversion_id: Some(convert_cart.conversion_id),
                user_id: convert_cart.convert_cart.customer_id,
                seller_prices: convert_cart.convert_cart.prices,
                address: convert_cart.convert_cart.address,
                receiver_name: convert_cart.convert_cart.receiver_name,
                receiver_phone: convert_cart.convert_cart.receiver_phone,
                receiver_email: convert_cart.convert_cart.receiver_email,
                coupons: convert_cart.convert_cart.coupons,
                delivery_info: convert_cart.convert_cart.delivery_info,
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
            .use_coupon(Initiator::Superadmin, coupon_id, customer)
            .then(|res| match res {
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
            .and_then(move |res| {
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

        self.billing_microservice
            .create_invoice(Initiator::Superadmin, input.clone())
            .and_then(move |res| {
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
        let cluster_url = self.config.cluster.url.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        self.users_microservice
            .get(Some(user_id.into()), user_id)
            .and_then(move |user| {
                user.ok_or_else(|| {
                    error!(
                        "Sending notification to user can not be done. User with id: {} is not found.",
                        user_id
                    );
                    format_err!("User is not found in users microservice.")
                        .context(Error::NotFound)
                        .into()
                }).into_future()
            }).and_then(move |user| {
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
                notifications_microservice.order_create_for_user(Initiator::Superadmin, email)
            })
    }

    fn notify_store_create_order(&self, store_id: StoreId, order_slug: OrderSlug) -> impl Future<Item = (), Error = FailureError> {
        let cluster_url = self.config.cluster.url.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        self.stores_microservice
            .get(store_id, Visibility::Active)
            .and_then(move |store| {
                store
                    .ok_or_else(|| {
                        error!(
                            "Sending notification to store can not be done. Store with id: {} is not found.",
                            store_id
                        );
                        format_err!("Store is not found in stores microservice.")
                            .context(Error::NotFound)
                            .into()
                    }).into_future()
            }).and_then(move |store| {
                if let Some(store_email) = store.email {
                    let email = OrderCreateForStore {
                        store_email,
                        store_id: store_id.to_string(),
                        order_slug: order_slug.to_string(),
                        cluster_url,
                    };
                    Either::A(notifications_microservice.order_create_for_store(Initiator::Superadmin, email))
                } else {
                    Either::B(future::ok(()))
                }
            })
    }

    fn notify_user_update_order(
        &self,
        user_id: UserId,
        order_slug: OrderSlug,
        order_state: OrderState,
    ) -> impl Future<Item = (), Error = FailureError> {
        let cluster_url = self.config.cluster.url.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        self.users_microservice
            .get(Some(user_id.into()), user_id)
            .and_then(move |user| {
                user.ok_or_else(|| {
                    error!(
                        "Sending notification to user can not be done. User with id: {} is not found.",
                        user_id
                    );
                    format_err!("User is not found in users microservice.")
                        .context(Error::NotFound)
                        .into()
                }).into_future()
            }).and_then(move |user| {
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
                notifications_microservice.order_update_state_for_user(Initiator::Superadmin, email)
            })
    }

    fn notify_store_update_order(
        &self,
        store_id: StoreId,
        order_slug: OrderSlug,
        order_state: OrderState,
    ) -> impl Future<Item = (), Error = FailureError> {
        let cluster_url = self.config.cluster.url.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        self.stores_microservice
            .get(store_id, Visibility::Active)
            .and_then(move |store| {
                store
                    .ok_or_else(|| {
                        error!(
                            "Sending notification to store can not be done. Store with id: {} is not found.",
                            store_id
                        );
                        format_err!("Store is not found in stores microservice.")
                            .context(Error::NotFound)
                            .into()
                    }).into_future()
            }).and_then(move |store| {
                if let Some(store_email) = store.email {
                    let email = OrderUpdateStateForStore {
                        store_email,
                        store_id: store.id.to_string(),
                        order_slug: order_slug.to_string(),
                        order_state: order_state.to_string(),
                        cluster_url,
                    };
                    Either::A(notifications_microservice.order_update_state_for_store(Initiator::Superadmin, email))
                } else {
                    Either::B(future::ok(()))
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

            let orders_microservice = self.orders_microservice.clone();

            let order_id = order_info.order_id;

            let res = self
                .orders_microservice
                .get_order(Some(order_info.customer_id.into()), OrderIdentifier::Id(order_info.order_id))
                .and_then(move |order| {
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
                        Either::B(orders_microservice.set_order_state(Some(Initiator::Superadmin), OrderIdentifier::Id(order.id), payload))
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
        let orders_microservice = self.orders_microservice.clone();
        self.orders_microservice
            .get_order(None, OrderIdentifier::Slug(order_slug))
            .and_then(move |order| {
                order
                    .ok_or(
                        format_err!("Order is not found in orders microservice! slug: {}", order_slug)
                            .context(Error::NotFound)
                            .into(),
                    ).into_future()
            }).and_then(move |order| {
                if order.state == order_state {
                    // if this status already set, do not update
                    info!("order slug: {:?} status: {:?} already set, do not update", order_slug, order_state);
                    Either::A(future::ok(None))
                } else {
                    info!(
                        "order slug: {:?} status: {:?} start request update on orders",
                        order_slug, order_state
                    );
                    Either::B(orders_microservice.set_order_state(
                        None,
                        OrderIdentifier::Slug(order_slug),
                        UpdateStatePayload {
                            state: order_state,
                            comment,
                            track_id,
                        },
                    ))
                }
            }).then(|res| match res {
                Ok(order) => Ok((self, order)),
                Err(e) => Err((self, e)),
            })
    }

    fn update_warehouse(self, orders: &[Option<Order>]) -> impl Future<Item = (Self, Vec<()>), Error = (Self, FailureError)> {
        debug!("Updating warehouses stock: {:?}", orders);

        let mut orders_futures = vec![];
        for order in orders {
            let warehouses_microservice = self.warehouses_microservice.clone();
            if let Some(order) = order {
                if order.state == OrderState::Paid {
                    debug!("Updating warehouses stock with product id {}", order.product);
                    let order_quantity = order.quantity;
                    let res = warehouses_microservice
                        .find_by_product_id(Initiator::Superadmin, order.product)
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
                                    warehouses_microservice
                                        .set_product_in_warehouse(
                                            Initiator::Superadmin,
                                            stock.warehouse_id,
                                            stock.product_id,
                                            Quantity(new_quantity),
                                        ).map(|_| ()),
                                );
                            }
                            Either::B(future::ok(()))
                        }).map_err(|e| {
                            let err = e
                                .context("decrementing quantity in warehouses microservice failed.")
                                .context(Error::HttpClient)
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
        let orders_microservice = self.orders_microservice.clone();
        let billing_microservice = self.billing_microservice.clone();
        let fut = iter_ok::<_, ()>(log).for_each(move |e| match e {
            CreateOrderOperationStage::OrdersConvertCartComplete(conversion_id) => {
                debug!("Reverting cart convertion, conversion_id: {}", conversion_id);
                let result = orders_microservice
                    .revert_convert_cart(Initiator::Superadmin, ConvertCartRevert { conversion_id })
                    .then(|_| Ok(()));

                Box::new(result) as Box<Future<Item = (), Error = ()>>
            }

            CreateOrderOperationStage::BillingCreateInvoiceComplete(saga_id) => {
                debug!("Reverting create invoice, saga_id: {}", saga_id);
                let result = billing_microservice
                    .revert_create_invoice(Initiator::Superadmin, saga_id)
                    .then(|_| Ok(()));

                Box::new(result) as Box<Future<Item = (), Error = ()>>
            }

            _ => Box::new(future::ok(())) as Box<Future<Item = (), Error = ()>>,
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

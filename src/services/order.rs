use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use futures;
use futures::future::join_all;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;

use serde_json;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;

use config;
use errors::Error;
use models::*;
use services::types::ServiceFuture;

pub trait OrderService {
    fn create(self, input: ConvertCart) -> ServiceFuture<Box<OrderService>, Option<BillingOrders>>;
    fn set_paid(&self, order_ids: Vec<OrderInfo>) -> Box<Future<Item = String, Error = FailureError>>;
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

    fn convert_cart(self, input: &ConvertCart) -> ServiceFuture<Self, Vec<Order>> {
        // Create Order
        debug!("Converting cart, input: {:?}", input);
        let log = self.log.clone();
        let customer_id = input.customer_id;
        log.lock()
            .unwrap()
            .push(CreateOrderOperationStage::OrdersConvertCartStart(customer_id));

        let mut headers = Headers::new();
        if let Some(ref user_id) = self.user_id {
            headers.set(Authorization(user_id.to_string()));
        };

        let orders_url = self.config.service_url(StqService::Orders);
        let client = self.http_client.clone();

        let res = serde_json::to_string(input)
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
                    .push(CreateOrderOperationStage::OrdersConvertCartComplete(customer_id));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_invoice(self, input: CreateInvoice) -> ServiceFuture<Self, BillingOrders> {
        // Create invoice
        debug!("Creating invoice, input: {:?}", input);
        let log = self.log.clone();

        let saga_id = input.saga_id;
        log.lock()
            .unwrap()
            .push(CreateOrderOperationStage::BillingCreateInvoiceStart(saga_id));

        let mut headers = Headers::new();
        if let Some(ref user_id) = self.user_id {
            headers.set(Authorization(user_id.to_string()));
        };

        let billing_url = self.config.service_url(StqService::Billing);
        let client = self.http_client.clone();

        let res = serde_json::to_string(&input)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<String>(Method::Post, format!("{}/invoices", billing_url), Some(body), Some(headers))
                    .map_err(|e| {
                        format_err!("Creating invoice in billing microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
                    .map(|url| BillingOrders { orders: input.orders, url })
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
    fn create_happy(self, input: ConvertCart) -> ServiceFuture<Self, BillingOrders> {
        Box::new(self.convert_cart(&input).and_then(move |(s, orders)| {
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
                CreateOrderOperationStage::OrdersConvertCartStart(customer_id) => {
                    debug!("Reverting cart convertion, customer_id: {:?}", customer_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can revert orders

                        s.http_client
                            .request::<CartHash>(
                                Method::Delete,
                                format!(
                                    "{}/{}/by-customer-id/{}",
                                    s.config.service_url(StqService::Orders),
                                    StqModel::Order.to_url(),
                                    customer_id.0.clone(),
                                ),
                                None,
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
                    debug!("Reverting create invoice, saga_id: {:?}", saga_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can revert invoice

                        s.http_client
                            .request::<UserId>(
                                Method::Delete,
                                format!("{}/invoices/{}", s.config.service_url(StqService::Billing), saga_id.0.clone(),),
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
    fn create(self, input: ConvertCart) -> ServiceFuture<Box<OrderService>, Option<BillingOrders>> {
        Box::new(
            self.create_happy(input.clone())
                .map(|(s, order)| (Box::new(s) as Box<OrderService>, Some(order)))
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

    fn set_paid(&self, orders_info: Vec<OrderInfo>) -> Box<Future<Item = String, Error = FailureError>> {
        // set paid
        debug!("Set orders paid : {:?}", orders_info);

        let client = self.http_client.clone();
        let orders_url = self.config.service_url(StqService::Orders);

        let orders_futures: Vec<Box<Future<Item = (), Error = FailureError>>> = orders_info
            .into_iter()
            .map(|order_info| {
                let payload = OrderStatusPaid::new();

                let body = serde_json::to_string(&payload).unwrap_or_default();
                let url = format!("{}/{}/by-id/{}/status", orders_url, StqModel::Order.to_url(), order_info.order_id.0);
                let mut headers = Headers::new();
                headers.set(Authorization(order_info.customer_id.0.to_string()));

                let res = client
                    .request::<Option<Order>>(Method::Post, url, Some(body), Some(headers))
                    .map_err(|e| {
                        format_err!("Setting paid in orders microservice error occured.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
                    .map(|_| ());
                Box::new(res) as Box<Future<Item = (), Error = FailureError>>
            })
            .collect();

        Box::new(join_all(orders_futures).map(|_| "Ok".to_string()))
    }
}

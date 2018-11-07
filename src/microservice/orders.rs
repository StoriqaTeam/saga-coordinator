use hyper::header::{Authorization, Headers};
use hyper::Method;

use stq_api::orders::{BuyNow, Order};
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::ApiFuture;

use config;
use http::{HttpClient, HttpClientWithDefaultHeaders};
use models::*;

pub trait OrdersMicroservice {
    fn cloned(&self) -> Box<OrdersMicroservice>;
    fn with_superadmin(&self) -> Box<OrdersMicroservice>;
    fn with_user(&self, user: UserId) -> Box<OrdersMicroservice>;
    fn convert_cart(&self, payload: ConvertCartPayload) -> ApiFuture<Vec<Order>>;
    fn get_order(&self, order_id: OrderIdentifier) -> ApiFuture<Option<Order>>;
    fn set_order_state(&self, order_id: OrderIdentifier, payload: UpdateStatePayload) -> ApiFuture<Option<Order>>;
    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>>;
    fn revert_convert_cart(&self, payload: ConvertCartRevert) -> ApiFuture<CartHash>;
}

pub struct OrdersMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl OrdersMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn orders_url(&self) -> String {
        self.config.service_url(StqService::Orders)
    }
}

impl OrdersMicroservice for OrdersMicroserviceImpl {
    fn cloned(&self) -> Box<OrdersMicroservice> {
        Box::new(OrdersMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn with_superadmin(&self) -> Box<OrdersMicroservice> {
        Box::new(OrdersMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn with_user(&self, user: UserId) -> Box<OrdersMicroservice> {
        let mut headers = Headers::new();
        headers.set(Authorization(user.0.to_string()));

        let http_client = HttpClientWithDefaultHeaders::new(self.http_client.cloned(), headers);

        Box::new(OrdersMicroserviceImpl {
            http_client: Box::new(http_client),
            config: self.config.clone(),
        })
    }

    fn convert_cart(&self, payload: ConvertCartPayload) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_from_cart", self.orders_url(), StqModel::Order.to_url());
        super::request::<_, ConvertCartPayload, Vec<Order>>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
    }

    fn get_order(&self, order_id: OrderIdentifier) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        super::request::<_, (), Option<Order>>(self.http_client.cloned(), Method::Get, url, None, None)
    }

    fn set_order_state(&self, order_id: OrderIdentifier, payload: UpdateStatePayload) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/status/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        super::request::<_, UpdateStatePayload, Option<Order>>(self.http_client.cloned(), Method::Put, url, Some(payload), None)
    }

    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_buy_now", self.orders_url(), StqModel::Order.to_url(),);

        super::request::<_, BuyNowPayload, Vec<Order>>(
            self.http_client.cloned(),
            Method::Post,
            url,
            Some(BuyNowPayload { conversion_id, buy_now }),
            None,
        )
    }

    fn revert_convert_cart(&self, payload: ConvertCartRevert) -> ApiFuture<CartHash> {
        let url = format!("{}/{}/create_buy_now/revert", self.orders_url(), StqModel::Order.to_url(),);

        super::request::<_, ConvertCartRevert, CartHash>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
    }
}

fn order_identifier_route(id: &OrderIdentifier) -> String {
    use self::OrderIdentifier::*;

    match id {
        Id(id) => format!("by-id/{}", id),
        Slug(slug) => format!("by-slug/{}", slug),
    }
}

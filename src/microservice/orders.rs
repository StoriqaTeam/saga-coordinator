use hyper::Method;

use stq_api::orders::{BuyNow, Order};
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;
use models::*;

pub trait OrdersMicroservice {
    fn convert_cart(&self, payload: ConvertCartPayload) -> ApiFuture<Vec<Order>>;
    fn get_order(&self, initiator: Option<Initiator>, order_id: OrderIdentifier) -> ApiFuture<Option<Order>>;
    fn set_order_state(
        &self,
        initiator: Option<Initiator>,
        order_id: OrderIdentifier,
        payload: UpdateStatePayload,
    ) -> ApiFuture<Option<Order>>;
    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>>;
    fn revert_convert_cart(&self, initiator: Initiator, payload: ConvertCartRevert) -> ApiFuture<CartHash>;
}

pub struct OrdersMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> OrdersMicroservice for OrdersMicroserviceImpl<T> {
    fn convert_cart(&self, payload: ConvertCartPayload) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_from_cart", self.orders_url(), StqModel::Order.to_url());
        super::request::<_, ConvertCartPayload, Vec<Order>>(self.http_client.clone(), Method::Post, url, Some(payload), None)
    }

    fn get_order(&self, initiator: Option<Initiator>, order_id: OrderIdentifier) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        super::request::<_, (), Option<Order>>(self.http_client.clone(), Method::Get, url, None, initiator.map(Into::into))
    }

    fn set_order_state(
        &self,
        initiator: Option<Initiator>,
        order_id: OrderIdentifier,
        payload: UpdateStatePayload,
    ) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/status/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );
        super::request::<_, UpdateStatePayload, Option<Order>>(
            self.http_client.clone(),
            Method::Put,
            url,
            Some(payload),
            initiator.map(Into::into),
        )
    }

    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_buy_now", self.orders_url(), StqModel::Order.to_url(),);

        super::request::<_, BuyNowPayload, Vec<Order>>(
            self.http_client.clone(),
            Method::Post,
            url,
            Some(BuyNowPayload { conversion_id, buy_now }),
            None,
        )
    }

    fn revert_convert_cart(&self, initiator: Initiator, payload: ConvertCartRevert) -> ApiFuture<CartHash> {
        let url = format!("{}/{}/create_buy_now/revert", self.orders_url(), StqModel::Order.to_url(),);
        let headers = initiator.into();
        super::request::<_, ConvertCartRevert, CartHash>(self.http_client.clone(), Method::Post, url, Some(payload), Some(headers))
    }
}

impl<T: 'static + HttpClient + Clone> OrdersMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn orders_url(&self) -> String {
        self.config.service_url(StqService::Orders)
    }
}

fn order_identifier_route(id: &OrderIdentifier) -> String {
    use self::OrderIdentifier::*;

    match id {
        Id(id) => format!("by-id/{}", id),
        Slug(slug) => format!("by-slug/{}", slug),
    }
}

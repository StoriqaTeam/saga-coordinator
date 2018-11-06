use std::collections::HashMap;

use futures::{Future, IntoFuture};
use hyper::header::Headers;
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;

use stq_api::orders::*;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_static_resources::OrderState;
use stq_types::*;

use super::ApiFuture;

use config;
use http::HttpClient;

pub trait OrdersMicroservice {
    fn cloned(&self) -> Box<OrdersMicroservice>;
    fn superadmin(&self) -> Box<OrdersMicroservice>;
    fn convert_cart(
        &self,
        conversion_id: Option<ConversionId>,
        user_id: UserId,
        seller_prices: HashMap<ProductId, ProductSellerPrice>,
        address: AddressFull,
        receiver_name: String,
        receiver_phone: String,
        receiver_email: String,
        coupons: HashMap<CouponId, CouponInfo>,
        delivery_info: HashMap<ProductId, DeliveryInfo>,
    ) -> ApiFuture<Vec<Order>>;
    fn get_order(&self, order_id: OrderIdentifier) -> ApiFuture<Option<Order>>;
    fn set_order_state(
        &self,
        order_id: OrderIdentifier,
        state: OrderState,
        comment: Option<String>,
        track_id: Option<String>,
    ) -> ApiFuture<Option<Order>>;
    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>>;
}

pub struct OrdersMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConvertCartPayload {
    pub conversion_id: Option<ConversionId>,
    pub user_id: UserId,
    pub receiver_name: String,
    pub receiver_phone: String,
    pub receiver_email: String,
    #[serde(flatten)]
    pub address: AddressFull,
    pub seller_prices: HashMap<ProductId, ProductSellerPrice>,
    pub coupons: HashMap<CouponId, CouponInfo>,
    pub delivery_info: HashMap<ProductId, DeliveryInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatePayload {
    pub state: OrderState,
    pub track_id: Option<String>,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuyNowPayload {
    pub conversion_id: Option<ConversionId>,
    #[serde(flatten)]
    pub buy_now: BuyNow,
}

impl OrdersMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn request<T: Serialize, S: for<'a> Deserialize<'a> + 'static + Send>(
        &self,
        method: Method,
        url: String,
        payload: Option<T>,
        headers: Option<Headers>,
    ) -> ApiFuture<S> {
        let body = if let Some(payload) = payload {
            serde_json::to_string::<T>(&payload).map(Some)
        } else {
            Ok(None)
        };

        let http_client = self.http_client.cloned();

        let result = body
            .into_future()
            .map_err(From::from)
            .and_then(move |serialized_body| http_client.request(method, url, serialized_body, headers))
            .and_then(|response| response.parse::<S>().into_future());
        Box::new(result)
    }
}

impl OrdersMicroservice for OrdersMicroserviceImpl {
    fn cloned(&self) -> Box<OrdersMicroservice> {
        Box::new(OrdersMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn superadmin(&self) -> Box<OrdersMicroservice> {
        Box::new(OrdersMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn convert_cart(
        &self,
        conversion_id: Option<ConversionId>,
        user_id: UserId,
        seller_prices: HashMap<ProductId, ProductSellerPrice>,
        address: AddressFull,
        receiver_name: String,
        receiver_phone: String,
        receiver_email: String,
        coupons: HashMap<CouponId, CouponInfo>,
        delivery_info: HashMap<ProductId, DeliveryInfo>,
    ) -> ApiFuture<Vec<Order>> {
        let orders_url = self.config.service_url(StqService::Orders);
        let url = format!("{}/{}/create_from_cart", orders_url, StqModel::Order.to_url());
        self.request::<_, Vec<Order>>(
            Method::Post,
            url,
            Some(ConvertCartPayload {
                conversion_id,
                user_id,
                seller_prices,
                address,
                receiver_name,
                receiver_phone,
                receiver_email,
                coupons,
                delivery_info,
            }),
            None,
        )
    }

    fn get_order(&self, order_id: OrderIdentifier) -> ApiFuture<Option<Order>> {
        let orders_url = self.config.service_url(StqService::Orders);
        let url = format!("{}/{}/{}", orders_url, StqModel::Order.to_url(), order_identifier_route(&order_id),);

        self.request::<(), Option<Order>>(Method::Get, url, None, None)
    }

    fn set_order_state(
        &self,
        order_id: OrderIdentifier,
        state: OrderState,
        comment: Option<String>,
        track_id: Option<String>,
    ) -> ApiFuture<Option<Order>> {
        let orders_url = self.config.service_url(StqService::Orders);
        let url = format!(
            "{}/{}/status/{}",
            orders_url,
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        self.request::<_, Option<Order>>(Method::Put, url, Some(UpdateStatePayload { state, comment, track_id }), None)
    }

    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>> {
        let orders_url = self.config.service_url(StqService::Orders);
        let url = format!("{}/{}/create_buy_now", orders_url, StqModel::Order.to_url(),);

        self.request::<_, Vec<Order>>(Method::Post, url, Some(BuyNowPayload { conversion_id, buy_now }), None)
    }
}

fn order_identifier_route(id: &OrderIdentifier) -> String {
    use self::OrderIdentifier::*;

    match id {
        Id(id) => format!("by-id/{}", id),
        Slug(slug) => format!("by-slug/{}", slug),
    }
}

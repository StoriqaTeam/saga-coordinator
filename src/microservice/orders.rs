use std::collections::HashMap;

use futures::{Future, IntoFuture};
use hyper::header::{Authorization, Headers};
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;

use stq_api::orders::{AddressFull, BuyNow, CouponInfo, DeliveryInfo, Order};
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
    fn set_order_state(&self, order_id: OrderIdentifier, payload: UpdateStatePayload) -> ApiFuture<Option<Order>>;
    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>>;
    fn revert_convert_cart(&self, payload: ConvertCartRevert) -> ApiFuture<CartHash>;
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
        let url = format!("{}/{}/create_from_cart", self.orders_url(), StqModel::Order.to_url());
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
        let url = format!(
            "{}/{}/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        self.request::<(), Option<Order>>(Method::Get, url, None, None)
    }

    fn set_order_state(&self, order_id: OrderIdentifier, payload: UpdateStatePayload) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/status/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        self.request::<_, Option<Order>>(Method::Put, url, Some(payload), None)
    }

    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_buy_now", self.orders_url(), StqModel::Order.to_url(),);

        self.request::<_, Vec<Order>>(Method::Post, url, Some(BuyNowPayload { conversion_id, buy_now }), None)
    }

    fn revert_convert_cart(&self, payload: ConvertCartRevert) -> ApiFuture<CartHash> {
        let url = format!("{}/{}/create_buy_now/revert", self.orders_url(), StqModel::Order.to_url(),);

        self.request::<_, CartHash>(Method::Post, url, Some(payload), None)
    }
}

fn order_identifier_route(id: &OrderIdentifier) -> String {
    use self::OrderIdentifier::*;

    match id {
        Id(id) => format!("by-id/{}", id),
        Slug(slug) => format!("by-slug/{}", slug),
    }
}

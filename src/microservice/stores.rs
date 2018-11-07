use futures::{Future, IntoFuture};
use hyper::header::Headers;
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::ApiFuture;

use config;
use http::HttpClient;
use models::*;

pub trait StoresMicroservice {
    fn cloned(&self) -> Box<StoresMicroservice>;
    fn with_superadmin(&self) -> Box<StoresMicroservice>;
    fn use_coupon(&self, coupon: CouponId, user: UserId) -> ApiFuture<UsedCoupon>;
    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>>;
}

pub struct StoresMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl StoresMicroservice for StoresMicroserviceImpl {
    fn cloned(&self) -> Box<StoresMicroservice> {
        Box::new(StoresMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn with_superadmin(&self) -> Box<StoresMicroservice> {
        Box::new(StoresMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>> {
        let url = format!("{}/{}/{}", self.stores_url(), StqModel::Store.to_url(), store);
        self.request::<(), Option<Store>>(Method::Get, url, None, None)
    }

    fn use_coupon(&self, coupon_id: CouponId, user: UserId) -> ApiFuture<UsedCoupon> {
        let url = format!("{}/{}/{}/users/{}", self.stores_url(), StqModel::Coupon.to_url(), coupon_id, user);
        self.request::<(), UsedCoupon>(Method::Post, url, None, None)
    }
}

impl StoresMicroserviceImpl {
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

    fn stores_url(&self) -> String {
        self.config.service_url(StqService::Stores)
    }
}

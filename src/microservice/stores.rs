use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;
use models::*;

pub trait StoresMicroservice {
    fn use_coupon(&self, initiator: Initiator, coupon: CouponId, user: UserId) -> ApiFuture<UsedCoupon>;
    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>>;
}

pub struct StoresMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> StoresMicroservice for StoresMicroserviceImpl<T> {
    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>> {
        let url = format!("{}/{}/{}", self.stores_url(), StqModel::Store.to_url(), store);
        super::request::<_, (), Option<Store>>(self.http_client.clone(), Method::Get, url, None, None)
    }

    fn use_coupon(&self, initiator: Initiator, coupon_id: CouponId, user: UserId) -> ApiFuture<UsedCoupon> {
        let url = format!("{}/{}/{}/users/{}", self.stores_url(), StqModel::Coupon.to_url(), coupon_id, user);
        super::request::<_, (), UsedCoupon>(self.http_client.clone(), Method::Post, url, None, Some(initiator.into()))
    }
}

impl<T: 'static + HttpClient + Clone> StoresMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn stores_url(&self) -> String {
        self.config.service_url(StqService::Stores)
    }
}

use failure::Fail;
use futures::Future;
use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use errors::Error;
use http::HttpClient;
use models::*;

pub trait StoresMicroservice {
    fn delete_stores_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<StoresRole>>;
    fn create_stores_role(&self, initiator: Option<Initiator>, payload: NewRole<StoresRole>) -> ApiFuture<NewRole<StoresRole>>;
    fn delete_store(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<Store>;
    fn create_store(&self, initiator: Option<Initiator>, payload: NewStore) -> ApiFuture<Store>;
    fn use_coupon(&self, initiator: Initiator, coupon: CouponId, user: UserId) -> ApiFuture<UsedCoupon>;
    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>>;
}

pub struct StoresMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> StoresMicroservice for StoresMicroserviceImpl<T> {
    fn delete_stores_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<StoresRole>> {
        let url = format!("{}/roles/by-id/{}", self.stores_url(), role_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting role in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_stores_role(&self, initiator: Option<Initiator>, payload: NewRole<StoresRole>) -> ApiFuture<NewRole<StoresRole>> {
        let url = format!("{}/{}", self.stores_url(), StqModel::Role.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Creating role in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn delete_store(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<Store> {
        let url = format!("{}/{}/{}", self.stores_url(), StqModel::Store.to_url(), store_id);
        Box::new(
            super::request::<_, NewStore, Store>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(
                |e| {
                    e.context("Deleting store in stores microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                },
            ),
        )
    }

    fn create_store(&self, initiator: Option<Initiator>, payload: NewStore) -> ApiFuture<Store> {
        let url = format!("{}/{}", self.stores_url(), StqModel::Store.to_url());
        Box::new(
            super::request::<_, NewStore, Store>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Creating store in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }
    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>> {
        let url = format!("{}/{}/{}", self.stores_url(), StqModel::Store.to_url(), store);
        Box::new(
            super::request::<_, (), Option<Store>>(self.http_client.clone(), Method::Get, url, None, None).map_err(|e| {
                e.context("Getting store in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn use_coupon(&self, initiator: Initiator, coupon_id: CouponId, user: UserId) -> ApiFuture<UsedCoupon> {
        let url = format!("{}/{}/{}/users/{}", self.stores_url(), StqModel::Coupon.to_url(), coupon_id, user);
        Box::new(
            super::request::<_, (), UsedCoupon>(self.http_client.clone(), Method::Post, url, None, Some(initiator.into())).map_err(|e| {
                e.context("Commit coupon for user in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
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

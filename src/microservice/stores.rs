use failure::Error as FailureError;
use failure::Fail;
use futures::{Future, IntoFuture};
use hyper::Method;
use serde_json;

use stq_http::client::HttpClient;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use errors::Error;
use models::*;

pub trait StoresMicroservice {
    fn delete_stores_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<StoresRole>>;
    fn create_stores_role(&self, initiator: Option<Initiator>, payload: NewRole<StoresRole>) -> ApiFuture<NewRole<StoresRole>>;
    fn delete_store(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<Store>;
    fn create_store(&self, initiator: Option<Initiator>, payload: NewStore) -> ApiFuture<Store>;
    fn use_coupon(&self, initiator: Initiator, coupon: CouponId, user: UserId) -> ApiFuture<UsedCoupon>;
    fn get(&self, store: StoreId) -> ApiFuture<Option<Store>>;
    fn set_store_moderation_status(&self, payload: StoreModerate) -> ApiFuture<Store>;
    fn send_to_moderation(&self, store_id: StoreId) -> ApiFuture<Store>;
    fn set_moderation_status_base_product(&self, payload: BaseProductModerate) -> ApiFuture<()>;
    fn send_to_moderation_base_product(&self, base_product_id: BaseProductId) -> ApiFuture<()>;
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

    fn set_store_moderation_status(&self, payload: StoreModerate) -> ApiFuture<Store> {
        let url = format!("{}/{}/moderate", self.stores_url(), StqModel::Store.to_url());

        Box::new(
            super::request::<_, StoreModerate, Store>(self.http_client.clone(), Method::Post, url, Some(payload), None).map_err(|e| {
                e.context("Set new status for store in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn send_to_moderation(&self, store_id: StoreId) -> ApiFuture<Store> {
        let url = format!("{}/{}/{}/moderation", self.stores_url(), StqModel::Store.to_url(), store_id);

        Box::new(
            super::request::<_, (), Store>(self.http_client.clone(), Method::Post, url, None, None).map_err(|e| {
                e.context("Send store to moderation in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn set_moderation_status_base_product(&self, payload: BaseProductModerate) -> ApiFuture<()> {
        let url = format!("{}/{}/moderate", self.stores_url(), StqModel::Store.to_url());
        let body = serde_json::to_string(&payload);
        let http_client = self.http_client.clone();

        Box::new(
            body.into_future()
                .map_err(FailureError::from)
                .and_then(move |serialized_body| {
                    http_client
                        .request(Method::Post, url, Some(serialized_body), None)
                        .map(|_| ())
                        .map_err(FailureError::from)
                }).map_err(|e| {
                    e.context("Set new status for store in stores microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                }),
        )
    }

    fn send_to_moderation_base_product(&self, base_product_id: BaseProductId) -> ApiFuture<()> {
        let url = format!(
            "{}/{}/{}/moderation",
            self.stores_url(),
            StqModel::BaseProduct.to_url(),
            base_product_id
        );

        Box::new(self.http_client.request(Method::Post, url, None, None).map(|_| ()).map_err(|e| {
            e.context("Send base_product to moderation in stores microservice failed.")
                .context(Error::HttpClient)
                .into()
        }))
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

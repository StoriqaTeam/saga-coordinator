use failure::Fail;
use futures::Future;
use hyper::Method;

use stq_http::client::HttpClient;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use errors::Error;
use models::*;
use services::parse_validation_errors;

pub trait StoresMicroservice {
    fn delete_stores_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<StoresRole>>;
    fn create_stores_role(&self, initiator: Option<Initiator>, payload: NewRole<StoresRole>) -> ApiFuture<NewRole<StoresRole>>;
    fn delete_store(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<Store>;
    fn create_store(&self, initiator: Option<Initiator>, payload: NewStore) -> ApiFuture<Store>;
    fn use_coupon(&self, initiator: Initiator, coupon: CouponId, user: UserId) -> ApiFuture<UsedCoupon>;
    fn get(&self, store: StoreId, visibility: Visibility) -> ApiFuture<Option<Store>>;
    fn set_store_moderation_status(&self, payload: StoreModerate) -> ApiFuture<Store>;
    fn send_to_moderation(&self, store_id: StoreId) -> ApiFuture<Store>;
    fn set_moderation_status_base_product(&self, payload: BaseProductModerate) -> ApiFuture<BaseProduct>;
    fn send_to_moderation_base_product(&self, base_product_id: BaseProductId) -> ApiFuture<BaseProduct>;
    fn get_moderators(&self, initiator: Initiator) -> ApiFuture<Vec<UserId>>;
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
    fn get(&self, store: StoreId, visibility: Visibility) -> ApiFuture<Option<Store>> {
        let url = format!(
            "{}/{}/{}?visibility={}",
            self.stores_url(),
            StqModel::Store.to_url(),
            store,
            visibility
        );
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
                parse_validation_errors(e.into(), &["store"])
                    .context("Set new status for store in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn send_to_moderation(&self, store_id: StoreId) -> ApiFuture<Store> {
        let url = format!("{}/{}/{}/moderation", self.stores_url(), StqModel::Store.to_url(), store_id);

        Box::new(
            super::request::<_, (), Store>(self.http_client.clone(), Method::Post, url, None, None).map_err(|e| {
                parse_validation_errors(e.into(), &["store"])
                    .context("Send store to moderation to moderation in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn set_moderation_status_base_product(&self, payload: BaseProductModerate) -> ApiFuture<BaseProduct> {
        let url = format!("{}/{}/moderate", self.stores_url(), StqModel::BaseProduct.to_url());

        Box::new(
            super::request::<_, BaseProductModerate, BaseProduct>(self.http_client.clone(), Method::Post, url, Some(payload), None)
                .map_err(|e| {
                    parse_validation_errors(e.into(), &["base_product"])
                        .context("Set new status for base_product in stores microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                }),
        )
    }

    fn send_to_moderation_base_product(&self, base_product_id: BaseProductId) -> ApiFuture<BaseProduct> {
        let url = format!(
            "{}/{}/{}/moderation",
            self.stores_url(),
            StqModel::BaseProduct.to_url(),
            base_product_id
        );

        Box::new(
            super::request::<_, (), BaseProduct>(self.http_client.clone(), Method::Post, url, None, None).map_err(|e| {
                parse_validation_errors(e.into(), &["base_product"])
                    .context("Send base_product to moderation in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn get_moderators(&self, initiator: Initiator) -> ApiFuture<Vec<UserId>> {
        let url = format!(
            "{}/{}/by-role/{}",
            self.stores_url(),
            StqModel::Role.to_url(),
            StoresRole::Moderator
        );

        Box::new(
            super::request::<_, (), Vec<UserId>>(self.http_client.clone(), Method::Get, url, None, Some(initiator.into())).map_err(|e| {
                e.context("Get moderators in stores microservice failed.")
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

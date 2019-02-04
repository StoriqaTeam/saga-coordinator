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

pub trait DeliveryMicroservice {
    fn delete_base_product(&self, initiator: Option<Initiator>, base_product_id: BaseProductId) -> ApiFuture<()>;
    fn delete_delivery_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<DeliveryRole>>;
    fn create_delivery_role(&self, initiator: Option<Initiator>, payload: NewRole<DeliveryRole>) -> ApiFuture<NewRole<DeliveryRole>>;
    fn upsert_shipping(&self, initiator: Option<Initiator>, base_product_id: BaseProductId, payload: NewShipping) -> ApiFuture<Shipping>;
}

pub struct DeliveryMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> DeliveryMicroservice for DeliveryMicroserviceImpl<T> {
    fn delete_base_product(&self, initiator: Option<Initiator>, base_product_id: BaseProductId) -> ApiFuture<()> {
        let url = format!("{}/{}/{}", self.delivery_url(), StqModel::BaseProduct.to_url(), base_product_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting base product in delivery microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn delete_delivery_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<DeliveryRole>> {
        let url = format!("{}/roles/by-id/{}", self.delivery_url(), role_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting role in delivery microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_delivery_role(&self, initiator: Option<Initiator>, payload: NewRole<DeliveryRole>) -> ApiFuture<NewRole<DeliveryRole>> {
        let url = format!("{}/{}", self.delivery_url(), StqModel::Role.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Creating role in delivery microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn upsert_shipping(&self, initiator: Option<Initiator>, base_product_id: BaseProductId, payload: NewShipping) -> ApiFuture<Shipping> {
        let url = format!("{}/products/{}", self.delivery_url(), base_product_id);
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Set shipping in delivery microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }
}

impl<T: 'static + HttpClient + Clone> DeliveryMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn delivery_url(&self) -> String {
        self.config.service_url(StqService::Delivery)
    }
}

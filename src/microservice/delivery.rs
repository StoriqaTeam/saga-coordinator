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
    fn delete_delivery_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<DeliveryRole>>;
    fn create_delivery_role(&self, initiator: Option<Initiator>, payload: NewRole<DeliveryRole>) -> ApiFuture<NewRole<DeliveryRole>>;
}

pub struct DeliveryMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> DeliveryMicroservice for DeliveryMicroserviceImpl<T> {
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
            ).map_err(|e| {
                e.context("Creating role in delivery microservice failed.")
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

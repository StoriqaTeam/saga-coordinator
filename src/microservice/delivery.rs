use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;
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
        super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into))
    }

    fn create_delivery_role(&self, initiator: Option<Initiator>, payload: NewRole<DeliveryRole>) -> ApiFuture<NewRole<DeliveryRole>> {
        let url = format!("{}/{}", self.delivery_url(), StqModel::Role.to_url());
        super::request(
            self.http_client.clone(),
            Method::Post,
            url,
            Some(payload),
            initiator.map(Into::into),
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

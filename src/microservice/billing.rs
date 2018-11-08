use hyper::Method;

use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;
use models::*;

pub trait BillingMicroservice {
    fn create_invoice(&self, initiator: Initiator, payload: CreateInvoice) -> ApiFuture<Invoice>;
    fn revert_create_invoice(&self, initiator: Initiator, saga_id: SagaId) -> ApiFuture<SagaId>;
}

pub struct BillingMicroserviceImpl<T: HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> BillingMicroservice for BillingMicroserviceImpl<T> {
    fn revert_create_invoice(&self, initiator: Initiator, saga_id: SagaId) -> ApiFuture<SagaId> {
        let url = format!("{}/invoices/by-saga-id/{}", self.billing_url(), saga_id.0);
        super::request::<_, (), SagaId>(self.http_client.clone(), Method::Delete, url, None, Some(initiator.into()))
    }

    fn create_invoice(&self, initiator: Initiator, payload: CreateInvoice) -> ApiFuture<Invoice> {
        let url = format!("{}/invoices", self.billing_url());
        super::request::<_, CreateInvoice, Invoice>(self.http_client.clone(), Method::Post, url, Some(payload), Some(initiator.into()))
    }
}

impl<T: HttpClient + Clone> BillingMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn billing_url(&self) -> String {
        self.config.service_url(StqService::Billing)
    }
}

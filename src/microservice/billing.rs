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

pub struct BillingMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl BillingMicroservice for BillingMicroserviceImpl {
    fn revert_create_invoice(&self, initiator: Initiator, saga_id: SagaId) -> ApiFuture<SagaId> {
        let url = format!("{}/invoices/by-saga-id/{}", self.billing_url(), saga_id.0);
        super::request::<_, (), SagaId>(self.http_client.cloned(), Method::Delete, url, None, Some(initiator.into()))
    }

    fn create_invoice(&self, initiator: Initiator, payload: CreateInvoice) -> ApiFuture<Invoice> {
        let url = format!("{}/invoices", self.billing_url());
        super::request::<_, CreateInvoice, Invoice>(self.http_client.cloned(), Method::Post, url, Some(payload), Some(initiator.into()))
    }
}

impl BillingMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn billing_url(&self) -> String {
        self.config.service_url(StqService::Billing)
    }
}

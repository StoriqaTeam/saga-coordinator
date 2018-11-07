use hyper::Method;

use stq_routes::service::Service as StqService;
use stq_types::*;

use super::ApiFuture;

use config;
use http::HttpClient;
use models::*;

pub trait BillingMicroservice {
    fn cloned(&self) -> Box<BillingMicroservice>;
    fn with_superadmin(&self) -> Box<BillingMicroservice>;
    fn create_invoice(&self, payload: CreateInvoice) -> ApiFuture<Invoice>;
    fn revert_create_invoice(&self, saga_id: SagaId) -> ApiFuture<SagaId>;
}

pub struct BillingMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl BillingMicroservice for BillingMicroserviceImpl {
    fn cloned(&self) -> Box<BillingMicroservice> {
        Box::new(BillingMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn with_superadmin(&self) -> Box<BillingMicroservice> {
        Box::new(BillingMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn revert_create_invoice(&self, saga_id: SagaId) -> ApiFuture<SagaId> {
        let url = format!("{}/invoices/by-saga-id/{}", self.billing_url(), saga_id.0);

        super::request::<_, (), SagaId>(self.http_client.cloned(), Method::Delete, url, None, None)
    }

    fn create_invoice(&self, payload: CreateInvoice) -> ApiFuture<Invoice> {
        let url = format!("{}/invoices", self.billing_url());
        super::request::<_, CreateInvoice, Invoice>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
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

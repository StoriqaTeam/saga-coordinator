use futures::{Future, IntoFuture};
use hyper::header::Headers;
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;

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

        self.request::<(), SagaId>(Method::Delete, url, None, None)
    }

    fn create_invoice(&self, payload: CreateInvoice) -> ApiFuture<Invoice> {
        let url = format!("{}/invoices", self.billing_url());
        self.request::<CreateInvoice, Invoice>(Method::Post, url, Some(payload), None)
    }
}

impl BillingMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn request<T: Serialize, S: for<'a> Deserialize<'a> + 'static + Send>(
        &self,
        method: Method,
        url: String,
        payload: Option<T>,
        headers: Option<Headers>,
    ) -> ApiFuture<S> {
        let body = if let Some(payload) = payload {
            serde_json::to_string::<T>(&payload).map(Some)
        } else {
            Ok(None)
        };

        let http_client = self.http_client.cloned();

        let result = body
            .into_future()
            .map_err(From::from)
            .and_then(move |serialized_body| http_client.request(method, url, serialized_body, headers))
            .and_then(|response| response.parse::<S>().into_future());
        Box::new(result)
    }

    fn billing_url(&self) -> String {
        self.config.service_url(StqService::Billing)
    }
}

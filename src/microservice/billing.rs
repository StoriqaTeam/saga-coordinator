use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;
use models::*;

pub trait BillingMicroservice {
    fn delete_store_merchant(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<MerchantId>;
    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<BillingRole>>;
    fn create_store_merchant(&self, initiator: Option<Initiator>, payload: CreateStoreMerchantPayload) -> ApiFuture<Merchant>;
    fn create_role(&self, initiator: Option<Initiator>, payload: NewRole<BillingRole>) -> ApiFuture<NewRole<BillingRole>>;
    fn create_invoice(&self, initiator: Initiator, payload: CreateInvoice) -> ApiFuture<Invoice>;
    fn revert_create_invoice(&self, initiator: Initiator, saga_id: SagaId) -> ApiFuture<SagaId>;
}

pub struct BillingMicroserviceImpl<T: HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> BillingMicroservice for BillingMicroserviceImpl<T> {
    fn delete_store_merchant(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<MerchantId> {
        let url = format!("{}/merchants/store/{}", self.billing_url(), store_id);
        super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into))
    }

    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<BillingRole>> {
        let url = format!("{}/roles/by-id/{}", self.billing_url(), role_id);
        super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into))
    }

    fn create_store_merchant(&self, initiator: Option<Initiator>, payload: CreateStoreMerchantPayload) -> ApiFuture<Merchant> {
        let url = format!("{}/merchants/store", self.billing_url());
        super::request(
            self.http_client.clone(),
            Method::Post,
            url,
            Some(payload),
            initiator.map(Into::into),
        )
    }

    fn create_role(&self, initiator: Option<Initiator>, payload: NewRole<BillingRole>) -> ApiFuture<NewRole<BillingRole>> {
        let url = format!("{}/{}", self.billing_url(), StqModel::Role.to_url());
        super::request(
            self.http_client.clone(),
            Method::Post,
            url,
            Some(payload),
            initiator.map(Into::into),
        )
    }

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

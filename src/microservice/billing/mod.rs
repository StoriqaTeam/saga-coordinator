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

pub trait BillingMicroservice {
    fn delete_user_merchant(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<MerchantId>;
    fn create_user_merchant(&self, initiator: Option<Initiator>, payload: CreateUserMerchantPayload) -> ApiFuture<Merchant>;
    fn delete_store_merchant(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<MerchantId>;
    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<BillingRole>>;
    fn create_store_merchant(&self, initiator: Option<Initiator>, payload: CreateStoreMerchantPayload) -> ApiFuture<Merchant>;
    fn create_role(&self, initiator: Option<Initiator>, payload: NewRole<BillingRole>) -> ApiFuture<NewRole<BillingRole>>;
    fn create_invoice(&self, initiator: Initiator, payload: CreateInvoice) -> ApiFuture<Invoice>;
    fn revert_create_invoice(&self, initiator: Initiator, saga_id: SagaId) -> ApiFuture<SagaId>;
    fn decline_order(&self, initiator: Initiator, order_id: OrderId) -> ApiFuture<()>;
    fn capture_order(&self, initiator: Initiator, order_id: OrderId) -> ApiFuture<()>;
    fn set_payment_state(&self, initiator: Option<Initiator>, order_id: OrderId, payload: OrderPaymentStateRequest) -> ApiFuture<()>;
}

pub struct BillingMicroserviceImpl<T: HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> BillingMicroservice for BillingMicroserviceImpl<T> {
    fn delete_user_merchant(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<MerchantId> {
        let url = format!("{}/merchants/user/{}", self.billing_url(), user_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting user merchant in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_user_merchant(&self, initiator: Option<Initiator>, payload: CreateUserMerchantPayload) -> ApiFuture<Merchant> {
        let url = format!("{}/merchants/user", self.billing_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Creating merchant in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn delete_store_merchant(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<MerchantId> {
        let url = format!("{}/merchants/store/{}", self.billing_url(), store_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting store merchant in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<BillingRole>> {
        let url = format!("{}/roles/by-id/{}", self.billing_url(), role_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting role in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_store_merchant(&self, initiator: Option<Initiator>, payload: CreateStoreMerchantPayload) -> ApiFuture<Merchant> {
        let url = format!("{}/merchants/store", self.billing_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Creating merchant in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_role(&self, initiator: Option<Initiator>, payload: NewRole<BillingRole>) -> ApiFuture<NewRole<BillingRole>> {
        let url = format!("{}/{}", self.billing_url(), StqModel::Role.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Creating role in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn revert_create_invoice(&self, initiator: Initiator, saga_id: SagaId) -> ApiFuture<SagaId> {
        let url = format!("{}/invoices/by-saga-id/{}", self.billing_url(), saga_id.0);
        Box::new(
            super::request::<_, (), SagaId>(self.http_client.clone(), Method::Delete, url, None, Some(initiator.into())).map_err(|e| {
                e.context("Reverting invoice creation in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_invoice(&self, initiator: Initiator, payload: CreateInvoice) -> ApiFuture<Invoice> {
        let url = format!("{}/invoices", self.billing_url());
        Box::new(
            super::request::<_, CreateInvoice, Invoice>(self.http_client.clone(), Method::Post, url, Some(payload), Some(initiator.into()))
                .map_err(|e| {
                    e.context("Creating invoice in billing microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                }),
        )
    }
    fn decline_order(&self, initiator: Initiator, order_id: OrderId) -> ApiFuture<()> {
        let url = format!("{}/orders/{}/decline", self.billing_url(), order_id);
        Box::new(
            super::request::<_, (), ()>(self.http_client.clone(), Method::Post, url, None, Some(initiator.into())).map_err(move |e| {
                e.context(format!("Declining order {} in billing microservice failed", order_id))
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }
    fn capture_order(&self, initiator: Initiator, order_id: OrderId) -> ApiFuture<()> {
        let url = format!("{}/orders/{}/capture", self.billing_url(), order_id);
        Box::new(
            super::request::<_, (), ()>(self.http_client.clone(), Method::Post, url, None, Some(initiator.into())).map_err(move |e| {
                e.context(format!("Capturing order {} in billing microservice failed", order_id))
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn set_payment_state(&self, initiator: Option<Initiator>, order_id: OrderId, payload: OrderPaymentStateRequest) -> ApiFuture<()> {
        let url = format!("{}/orders/{}/set_payment_state", self.billing_url(), order_id);
        Box::new(
            super::request::<_, OrderPaymentStateRequest, ()>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(move |e| {
                e.context(format!("Set payment state order {} in billing microservice failed", order_id))
                    .context(Error::HttpClient)
                    .into()
            }),
        )
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

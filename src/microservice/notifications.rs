use failure::Fail;
use futures::Future;
use hyper::Method;

use stq_http::client::HttpClient;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_static_resources::{
    ApplyEmailVerificationForUser, ApplyPasswordResetForUser, EmailVerificationForUser, OrderCreateForStore, OrderCreateForUser,
    OrderUpdateStateForStore, OrderUpdateStateForUser, PasswordResetForUser,
};

use super::{ApiFuture, Initiator};
use config;
use errors::Error;

pub trait NotificationsMicroservice {
    fn apply_email_verification(&self, initiator: Option<Initiator>, payload: ApplyEmailVerificationForUser) -> ApiFuture<()>;
    fn apply_password_reset(&self, initiator: Option<Initiator>, payload: ApplyPasswordResetForUser) -> ApiFuture<()>;
    fn password_reset(&self, initiator: Option<Initiator>, payload: PasswordResetForUser) -> ApiFuture<()>;
    fn email_verification(&self, initiator: Option<Initiator>, payload: EmailVerificationForUser) -> ApiFuture<()>;
    fn order_create_for_user(&self, initiator: Initiator, payload: OrderCreateForUser) -> ApiFuture<()>;
    fn order_create_for_store(&self, initiator: Initiator, payload: OrderCreateForStore) -> ApiFuture<()>;
    fn order_update_state_for_user(&self, initiator: Initiator, payload: OrderUpdateStateForUser) -> ApiFuture<()>;
    fn order_update_state_for_store(&self, initiator: Initiator, payload: OrderUpdateStateForStore) -> ApiFuture<()>;
}

pub struct NotificationsMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> NotificationsMicroservice for NotificationsMicroserviceImpl<T> {
    fn apply_email_verification(&self, initiator: Option<Initiator>, payload: ApplyEmailVerificationForUser) -> ApiFuture<()> {
        let url = format!("{}/{}/apply-email-verification", self.notifications_url(), StqModel::User.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| e.context("Sending notification failed.").context(Error::HttpClient).into()),
        )
    }

    fn apply_password_reset(&self, initiator: Option<Initiator>, payload: ApplyPasswordResetForUser) -> ApiFuture<()> {
        let url = format!("{}/{}/apply-password-reset", self.notifications_url(), StqModel::User.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| e.context("Sending notification failed.").context(Error::HttpClient).into()),
        )
    }

    fn password_reset(&self, initiator: Option<Initiator>, payload: PasswordResetForUser) -> ApiFuture<()> {
        let url = format!("{}/{}/password-reset", self.notifications_url(), StqModel::User.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| e.context("Sending notification failed.").context(Error::HttpClient).into()),
        )
    }

    fn email_verification(&self, initiator: Option<Initiator>, payload: EmailVerificationForUser) -> ApiFuture<()> {
        let url = format!("{}/{}/email-verification", self.notifications_url(), StqModel::User.to_url(),);
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Sending email to notifications microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn order_update_state_for_store(&self, initiator: Initiator, payload: OrderUpdateStateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-update-state", self.notifications_url());
        Box::new(
            super::request::<_, OrderUpdateStateForStore, ()>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                Some(initiator.into()),
            ).map_err(|e| {
                e.context("Sending order update for store in notifications microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn order_update_state_for_user(&self, initiator: Initiator, payload: OrderUpdateStateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-update-state", self.notifications_url());
        Box::new(
            super::request::<_, OrderUpdateStateForUser, ()>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                Some(initiator.into()),
            ).map_err(|e| {
                e.context("Sending order update for user in notifications microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn order_create_for_store(&self, initiator: Initiator, payload: OrderCreateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-create", self.notifications_url());
        Box::new(
            super::request::<_, OrderCreateForStore, ()>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                Some(initiator.into()),
            ).map_err(|e| {
                e.context("Sending order create for store in notifications microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn order_create_for_user(&self, initiator: Initiator, payload: OrderCreateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-create", self.notifications_url());
        Box::new(
            super::request::<_, OrderCreateForUser, ()>(self.http_client.clone(), Method::Post, url, Some(payload), Some(initiator.into()))
                .map_err(|e| {
                    e.context("Sending order create for user in notifications microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                }),
        )
    }
}

impl<T: 'static + HttpClient + Clone> NotificationsMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn notifications_url(&self) -> String {
        self.config.service_url(StqService::Notifications)
    }
}

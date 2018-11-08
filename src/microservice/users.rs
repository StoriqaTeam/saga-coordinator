use failure::Fail;
use futures::Future;
use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::enums::UsersRole;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use errors::Error;
use http::HttpClient;
use models::*;

pub trait UsersMicroservice {
    fn apply_email_verify_token(&self, initiator: Option<Initiator>, payload: EmailVerifyApply) -> ApiFuture<EmailVerifyApplyToken>;
    fn apply_password_reset_token(&self, initiator: Option<Initiator>, payload: PasswordResetApply) -> ApiFuture<ResetApplyToken>;
    fn create_password_reset_token(&self, initiator: Option<Initiator>, payload: ResetRequest) -> ApiFuture<String>;
    fn get_by_email(&self, initiator: Option<Initiator>, email: &str) -> ApiFuture<Option<User>>;
    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<UsersRole>>;
    fn delete_user(&self, initiator: Option<Initiator>, saga_id: SagaId) -> ApiFuture<User>;
    fn create_email_verify_token(&self, initiator: Option<Initiator>, payload: ResetRequest) -> ApiFuture<String>;
    fn create_role(&self, initiator: Option<Initiator>, payload: NewRole<UsersRole>) -> ApiFuture<NewRole<UsersRole>>;
    fn create_user(&self, initiator: Option<Initiator>, payload: SagaCreateProfile) -> ApiFuture<User>;
    fn get(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<Option<User>>;
}

pub struct UsersMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> UsersMicroservice for UsersMicroserviceImpl<T> {
    fn apply_email_verify_token(&self, initiator: Option<Initiator>, payload: EmailVerifyApply) -> ApiFuture<EmailVerifyApplyToken> {
        let url = format!(
            "{}/{}/email_verify_token?token={}",
            self.users_url(),
            StqModel::User.to_url(),
            payload.token
        );
        Box::new(
            super::request(self.http_client.clone(), Method::Put, url, Some(payload), initiator.map(Into::into)).map_err(|e| {
                e.context("Applying email verification token in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn apply_password_reset_token(&self, initiator: Option<Initiator>, payload: PasswordResetApply) -> ApiFuture<ResetApplyToken> {
        let url = format!("{}/{}/password_reset_token", self.users_url(), StqModel::User.to_url());
        Box::new(
            super::request(self.http_client.clone(), Method::Put, url, Some(payload), initiator.map(Into::into)).map_err(|e| {
                e.context("Applying password reset token in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_password_reset_token(&self, initiator: Option<Initiator>, payload: ResetRequest) -> ApiFuture<String> {
        let url = format!("{}/{}/password_reset_token", self.users_url(), StqModel::User.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Creating password reset token in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn get_by_email(&self, initiator: Option<Initiator>, email: &str) -> ApiFuture<Option<User>> {
        let url = format!("{}/{}/by_email?email={}", self.users_url(), StqModel::User.to_url(), email);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Get, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Receiving user from users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleId) -> ApiFuture<NewRole<UsersRole>> {
        let url = format!("{}/roles/by-id/{}", self.users_url(), role_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting role in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn delete_user(&self, initiator: Option<Initiator>, saga_id: SagaId) -> ApiFuture<User> {
        let url = format!("{}/user_by_saga_id/{}", self.users_url(), saga_id);
        super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into))
    }

    fn create_email_verify_token(&self, initiator: Option<Initiator>, payload: ResetRequest) -> ApiFuture<String> {
        let url = format!("{}/{}/email_verify_token", self.users_url(), StqModel::User.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Creating email verify token in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_role(&self, initiator: Option<Initiator>, payload: NewRole<UsersRole>) -> ApiFuture<NewRole<UsersRole>> {
        let url = format!("{}/{}", self.users_url(), StqModel::Role.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Creating role in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_user(&self, initiator: Option<Initiator>, payload: SagaCreateProfile) -> ApiFuture<User> {
        let url = format!("{}/{}", self.users_url(), StqModel::User.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            ).map_err(|e| {
                e.context("Creating user in users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn get(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<Option<User>> {
        let url = format!("{}/{}/{}", self.users_url(), StqModel::User.to_url(), user_id);
        Box::new(
            super::request::<_, (), Option<User>>(self.http_client.clone(), Method::Get, url, None, initiator.map(Into::into)).map_err(
                |e| {
                    e.context("Getting user in users microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                },
            ),
        )
    }
}

impl<T: 'static + HttpClient + Clone> UsersMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn users_url(&self) -> String {
        self.config.service_url(StqService::Users)
    }
}

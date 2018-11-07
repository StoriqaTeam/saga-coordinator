use hyper::header::{Authorization, Headers};
use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::ApiFuture;

use config;
use http::{HttpClient, HttpClientWithDefaultHeaders};
use models::*;

pub trait UsersMicroservice {
    fn cloned(&self) -> Box<UsersMicroservice>;
    fn with_superadmin(&self) -> Box<UsersMicroservice>;
    fn with_user(&self, user: UserId) -> Box<UsersMicroservice>;
    fn get(&self, user_id: UserId) -> ApiFuture<Option<User>>;
}

pub struct UsersMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl UsersMicroservice for UsersMicroserviceImpl {
    fn cloned(&self) -> Box<UsersMicroservice> {
        Box::new(UsersMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn with_superadmin(&self) -> Box<UsersMicroservice> {
        Box::new(UsersMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn with_user(&self, user: UserId) -> Box<UsersMicroservice> {
        let mut headers = Headers::new();
        headers.set(Authorization(user.0.to_string()));

        let http_client = HttpClientWithDefaultHeaders::new(self.http_client.cloned(), headers);

        Box::new(UsersMicroserviceImpl {
            http_client: Box::new(http_client),
            config: self.config.clone(),
        })
    }

    fn get(&self, user_id: UserId) -> ApiFuture<Option<User>> {
        let url = format!("{}/{}/{}", self.users_url(), StqModel::User.to_url(), user_id);

        super::request::<_, (), Option<User>>(self.http_client.cloned(), Method::Get, url, None, None)
    }
}

impl UsersMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn users_url(&self) -> String {
        self.config.service_url(StqService::Users)
    }
}

use hyper::Method;

use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;
use models::*;

pub trait UsersMicroservice {
    fn get(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<Option<User>>;
}

pub struct UsersMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl UsersMicroservice for UsersMicroserviceImpl {
    fn get(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<Option<User>> {
        let url = format!("{}/{}/{}", self.users_url(), StqModel::User.to_url(), user_id);

        super::request::<_, (), Option<User>>(self.http_client.cloned(), Method::Get, url, None, initiator.map(Into::into))
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

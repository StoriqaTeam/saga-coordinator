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

pub struct UsersMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> UsersMicroservice for UsersMicroserviceImpl<T> {
    fn get(&self, initiator: Option<Initiator>, user_id: UserId) -> ApiFuture<Option<User>> {
        let url = format!("{}/{}/{}", self.users_url(), StqModel::User.to_url(), user_id);

        super::request::<_, (), Option<User>>(self.http_client.clone(), Method::Get, url, None, initiator.map(Into::into))
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

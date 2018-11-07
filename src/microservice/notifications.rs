use futures::{Future, IntoFuture};
use hyper::header::Headers;
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;

use stq_routes::service::Service as StqService;
use stq_static_resources::{OrderCreateForStore, OrderCreateForUser, OrderUpdateStateForStore, OrderUpdateStateForUser};

use super::ApiFuture;

use config;
use http::HttpClient;

pub trait NotificationsMicroservice {
    fn cloned(&self) -> Box<NotificationsMicroservice>;
    fn with_superadmin(&self) -> Box<NotificationsMicroservice>;
    fn order_create_for_user(&self, payload: OrderCreateForUser) -> ApiFuture<()>;
    fn order_create_for_store(&self, payload: OrderCreateForStore) -> ApiFuture<()>;
    fn order_update_state_for_user(&self, payload: OrderUpdateStateForUser) -> ApiFuture<()>;
    fn order_update_state_for_store(&self, payload: OrderUpdateStateForStore) -> ApiFuture<()>;
}

pub struct NotificationsMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl NotificationsMicroservice for NotificationsMicroserviceImpl {
    fn cloned(&self) -> Box<NotificationsMicroservice> {
        Box::new(NotificationsMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn with_superadmin(&self) -> Box<NotificationsMicroservice> {
        Box::new(NotificationsMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn order_update_state_for_store(&self, payload: OrderUpdateStateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-update-state", self.notifications_url());
        self.request::<OrderUpdateStateForStore, ()>(Method::Post, url, Some(payload), None)
    }

    fn order_update_state_for_user(&self, payload: OrderUpdateStateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-update-state", self.notifications_url());
        self.request::<OrderUpdateStateForUser, ()>(Method::Post, url, Some(payload), None)
    }

    fn order_create_for_store(&self, payload: OrderCreateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-create", self.notifications_url());
        self.request::<OrderCreateForStore, ()>(Method::Post, url, Some(payload), None)
    }

    fn order_create_for_user(&self, payload: OrderCreateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-create", self.notifications_url());
        self.request::<OrderCreateForUser, ()>(Method::Post, url, Some(payload), None)
    }
}

impl NotificationsMicroserviceImpl {
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

    fn notifications_url(&self) -> String {
        self.config.service_url(StqService::Notifications)
    }
}

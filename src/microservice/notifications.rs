use hyper::Method;

use stq_routes::service::Service as StqService;
use stq_static_resources::{OrderCreateForStore, OrderCreateForUser, OrderUpdateStateForStore, OrderUpdateStateForUser};

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;

pub trait NotificationsMicroservice {
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
    fn order_update_state_for_store(&self, initiator: Initiator, payload: OrderUpdateStateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-update-state", self.notifications_url());
        super::request::<_, OrderUpdateStateForStore, ()>(
            self.http_client.clone(),
            Method::Post,
            url,
            Some(payload),
            Some(initiator.into()),
        )
    }

    fn order_update_state_for_user(&self, initiator: Initiator, payload: OrderUpdateStateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-update-state", self.notifications_url());
        super::request::<_, OrderUpdateStateForUser, ()>(self.http_client.clone(), Method::Post, url, Some(payload), Some(initiator.into()))
    }

    fn order_create_for_store(&self, initiator: Initiator, payload: OrderCreateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-create", self.notifications_url());
        super::request::<_, OrderCreateForStore, ()>(self.http_client.clone(), Method::Post, url, Some(payload), Some(initiator.into()))
    }

    fn order_create_for_user(&self, initiator: Initiator, payload: OrderCreateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-create", self.notifications_url());
        super::request::<_, OrderCreateForUser, ()>(self.http_client.clone(), Method::Post, url, Some(payload), Some(initiator.into()))
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

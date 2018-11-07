use hyper::Method;

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
        super::request::<_, OrderUpdateStateForStore, ()>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
    }

    fn order_update_state_for_user(&self, payload: OrderUpdateStateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-update-state", self.notifications_url());
        super::request::<_, OrderUpdateStateForUser, ()>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
    }

    fn order_create_for_store(&self, payload: OrderCreateForStore) -> ApiFuture<()> {
        let url = format!("{}/stores/order-create", self.notifications_url());
        super::request::<_, OrderCreateForStore, ()>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
    }

    fn order_create_for_user(&self, payload: OrderCreateForUser) -> ApiFuture<()> {
        let url = format!("{}/users/order-create", self.notifications_url());
        super::request::<_, OrderCreateForUser, ()>(self.http_client.cloned(), Method::Post, url, Some(payload), None)
    }
}

impl NotificationsMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn notifications_url(&self) -> String {
        self.config.service_url(StqService::Notifications)
    }
}

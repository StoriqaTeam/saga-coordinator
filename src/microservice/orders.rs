use failure::Fail;
use futures::Future;
use hyper::Method;

use stq_api::orders::Order;
use stq_http::client::HttpClient;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use errors::Error;
use models::*;
use services::parse_validation_errors;

pub trait OrdersMicroservice {
    fn convert_cart(&self, payload: ConvertCartPayload) -> ApiFuture<Vec<Order>>;
    fn get_order(&self, initiator: Option<Initiator>, order_id: OrderIdentifier) -> ApiFuture<Option<Order>>;
    fn set_order_state(
        &self,
        initiator: Option<Initiator>,
        order_id: OrderIdentifier,
        payload: UpdateStatePayload,
    ) -> ApiFuture<Option<Order>>;
    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>>;
    fn revert_convert_cart(&self, initiator: Initiator, payload: ConvertCartRevert) -> ApiFuture<CartHash>;
    fn create_role(&self, initiator: Option<Initiator>, role: RoleEntry<NewOrdersRole>) -> ApiFuture<RoleEntry<NewOrdersRole>>;
    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleEntryId) -> ApiFuture<RoleEntry<NewOrdersRole>>;
}

pub struct OrdersMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> OrdersMicroservice for OrdersMicroserviceImpl<T> {
    fn delete_role(&self, initiator: Option<Initiator>, role_id: RoleEntryId) -> ApiFuture<RoleEntry<NewOrdersRole>> {
        let url = format!("{}/roles/by-id/{}", self.orders_url(), role_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting role in orders microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }
    fn create_role(&self, initiator: Option<Initiator>, payload: RoleEntry<NewOrdersRole>) -> ApiFuture<RoleEntry<NewOrdersRole>> {
        let url = format!("{}/{}", self.orders_url(), StqModel::Role.to_url());
        Box::new(
            super::request::<_, RoleEntry<NewOrdersRole>, RoleEntry<NewOrdersRole>>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Creating role in orders microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }
    fn convert_cart(&self, payload: ConvertCartPayload) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_from_cart", self.orders_url(), StqModel::Order.to_url());
        Box::new(
            super::request::<_, ConvertCartPayload, Vec<Order>>(self.http_client.clone(), Method::Post, url, Some(payload), None).map_err(
                |e| {
                    parse_validation_errors(e.into(), &["order"])
                        .context("Converting cart in orders microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                },
            ),
        )
    }

    fn get_order(&self, initiator: Option<Initiator>, order_id: OrderIdentifier) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/{}",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );

        Box::new(
            super::request::<_, (), Option<Order>>(self.http_client.clone(), Method::Get, url, None, initiator.map(Into::into)).map_err(
                move |e| {
                    parse_validation_errors(e.into(), &["order"])
                        .context(format!("Getting order with id {:?} in orders microservice failed.", order_id))
                        .context(Error::HttpClient)
                        .into()
                },
            ),
        )
    }

    fn set_order_state(
        &self,
        initiator: Option<Initiator>,
        order_id: OrderIdentifier,
        payload: UpdateStatePayload,
    ) -> ApiFuture<Option<Order>> {
        let url = format!(
            "{}/{}/{}/status",
            self.orders_url(),
            StqModel::Order.to_url(),
            order_identifier_route(&order_id),
        );
        let order_state = payload.state;
        Box::new(
            super::request::<_, UpdateStatePayload, Option<Order>>(
                self.http_client.clone(),
                Method::Put,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(move |e| {
                parse_validation_errors(e.into(), &["order"])
                    .context(format!(
                        "Setting order with id {:?} state {} in orders microservice failed.",
                        order_id, order_state
                    ))
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_buy_now(&self, buy_now: BuyNow, conversion_id: Option<ConversionId>) -> ApiFuture<Vec<Order>> {
        let url = format!("{}/{}/create_buy_now", self.orders_url(), StqModel::Order.to_url(),);

        Box::new(
            super::request::<_, BuyNowPayload, Vec<Order>>(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(BuyNowPayload { conversion_id, buy_now }),
                None,
            )
            .map_err(|e| {
                parse_validation_errors(e.into(), &["order"])
                    .context("Create order from buy now data in orders microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn revert_convert_cart(&self, initiator: Initiator, payload: ConvertCartRevert) -> ApiFuture<CartHash> {
        let url = format!("{}/{}/create_buy_now/revert", self.orders_url(), StqModel::Order.to_url(),);
        let headers = initiator.into();
        Box::new(
            super::request::<_, ConvertCartRevert, CartHash>(self.http_client.clone(), Method::Post, url, Some(payload), Some(headers))
                .map_err(|e| {
                    e.context("Revert convert cart in orders microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                }),
        )
    }
}

impl<T: 'static + HttpClient + Clone> OrdersMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn orders_url(&self) -> String {
        self.config.service_url(StqService::Orders)
    }
}

fn order_identifier_route(id: &OrderIdentifier) -> String {
    use self::OrderIdentifier::*;

    match id {
        Id(id) => format!("by-id/{}", id),
        Slug(slug) => format!("by-slug/{}", slug),
    }
}

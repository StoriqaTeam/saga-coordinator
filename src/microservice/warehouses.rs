use failure::Fail;
use futures::Future;
use hyper::Method;

use stq_api::warehouses::{Stock, StockSetPayload};
use stq_http::client::HttpClient;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use errors::Error;
use models::*;

pub trait WarehousesMicroservice {
    fn delete_warehouse_role(&self, initiator: Option<Initiator>, role_id: RoleEntryId) -> ApiFuture<RoleEntry<NewWarehouseRole>>;
    fn create_warehouse_role(
        &self,
        initiator: Option<Initiator>,
        payload: RoleEntry<NewWarehouseRole>,
    ) -> ApiFuture<RoleEntry<NewWarehouseRole>>;
    fn find_by_product_id(&self, initiator: Initiator, product_id: ProductId) -> ApiFuture<Vec<Stock>>;
    fn set_product_in_warehouse(
        &self,
        initiator: Initiator,
        warehouse_id: WarehouseId,
        product_id: ProductId,
        quantity: Quantity,
    ) -> ApiFuture<Stock>;
    fn find_by_store_id(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<Vec<Warehouse>>;
}

pub struct WarehousesMicroserviceImpl<T: 'static + HttpClient + Clone> {
    http_client: T,
    config: config::Config,
}

impl<T: 'static + HttpClient + Clone> WarehousesMicroservice for WarehousesMicroserviceImpl<T> {
    fn delete_warehouse_role(&self, initiator: Option<Initiator>, role_id: RoleEntryId) -> ApiFuture<RoleEntry<NewWarehouseRole>> {
        let url = format!("{}/roles/by-id/{}", self.warehouses_url(), role_id);
        Box::new(
            super::request::<_, (), _>(self.http_client.clone(), Method::Delete, url, None, initiator.map(Into::into)).map_err(|e| {
                e.context("Deleting role in warehouses microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn create_warehouse_role(
        &self,
        initiator: Option<Initiator>,
        payload: RoleEntry<NewWarehouseRole>,
    ) -> ApiFuture<RoleEntry<NewWarehouseRole>> {
        let url = format!("{}/{}", self.warehouses_url(), StqModel::Role.to_url());
        Box::new(
            super::request(
                self.http_client.clone(),
                Method::Post,
                url,
                Some(payload),
                initiator.map(Into::into),
            )
            .map_err(|e| {
                e.context("Creating role in warehouses microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }
    fn set_product_in_warehouse(
        &self,
        initiator: Initiator,
        warehouse_id: WarehouseId,
        product_id: ProductId,
        quantity: Quantity,
    ) -> ApiFuture<Stock> {
        let url = format!(
            "{}/warehouses/{}/products/{}",
            self.warehouses_url(),
            warehouse_identifier_route(&WarehouseIdentifier::Id(warehouse_id)),
            product_id
        );

        Box::new(
            super::request::<_, StockSetPayload, Stock>(
                self.http_client.clone(),
                Method::Put,
                url,
                Some(StockSetPayload { quantity }),
                Some(initiator.into()),
            )
            .map_err(|e| {
                e.context("Setting product quantity in warehouses microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn find_by_product_id(&self, initiator: Initiator, product_id: ProductId) -> ApiFuture<Vec<Stock>> {
        let url = format!("{}/stocks/by-product-id/{}", self.warehouses_url(), product_id);
        Box::new(
            super::request::<_, (), Vec<Stock>>(self.http_client.clone(), Method::Get, url, None, Some(initiator.into())).map_err(|e| {
                e.context("Find stocks in warehouses microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }),
        )
    }

    fn find_by_store_id(&self, initiator: Option<Initiator>, store_id: StoreId) -> ApiFuture<Vec<Warehouse>> {
        let url = format!("{}/warehouses/by-store/{}", self.warehouses_url(), store_id);
        Box::new(
            super::request::<_, (), Vec<Warehouse>>(self.http_client.clone(), Method::Get, url, None, initiator.map(Initiator::into))
                .map_err(|e| {
                    e.context("Find warehouses in warehouses microservice failed.")
                        .context(Error::HttpClient)
                        .into()
                }),
        )
    }
}

impl<T: 'static + HttpClient + Clone> WarehousesMicroserviceImpl<T> {
    pub fn new(http_client: T, config: config::Config) -> Self {
        Self { http_client, config }
    }

    fn warehouses_url(&self) -> String {
        self.config.service_url(StqService::Warehouses)
    }
}

fn warehouse_identifier_route(id: &WarehouseIdentifier) -> String {
    use self::WarehouseIdentifier::*;

    match id {
        Id(id) => format!("by-id/{}", id),
        Slug(slug) => format!("by-slug/{}", slug),
    }
}

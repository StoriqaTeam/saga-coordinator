use hyper::Method;

use stq_api::warehouses::{Stock, StockSetPayload};
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::{ApiFuture, Initiator};

use config;
use http::HttpClient;

pub trait WarehousesMicroservice {
    fn find_by_product_id(&self, initiator: Initiator, product_id: ProductId) -> ApiFuture<Vec<Stock>>;
    fn set_product_in_warehouse(
        &self,
        initiator: Initiator,
        warehouse_id: WarehouseId,
        product_id: ProductId,
        quantity: Quantity,
    ) -> ApiFuture<Stock>;
}

pub struct WarehousesMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl WarehousesMicroservice for WarehousesMicroserviceImpl {
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

        super::request::<_, StockSetPayload, Stock>(
            self.http_client.cloned(),
            Method::Put,
            url,
            Some(StockSetPayload { quantity }),
            Some(initiator.into()),
        )
    }

    fn find_by_product_id(&self, initiator: Initiator, product_id: ProductId) -> ApiFuture<Vec<Stock>> {
        let url = format!("{}/stocks/by-product-id/{}", self.warehouses_url(), product_id);
        super::request::<_, (), Vec<Stock>>(self.http_client.cloned(), Method::Get, url, None, Some(initiator.into()))
    }
}

impl WarehousesMicroserviceImpl {
    pub fn new(http_client: Box<HttpClient>, config: config::Config) -> Self {
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

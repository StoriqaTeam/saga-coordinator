use futures::{Future, IntoFuture};
use hyper::header::Headers;
use hyper::Method;
use serde::de::Deserialize;
use serde::ser::Serialize;

use stq_api::warehouses::{Stock, StockSetPayload};
use stq_routes::service::Service as StqService;
use stq_types::*;

use super::ApiFuture;

use config;
use http::HttpClient;

pub trait WarehousesMicroservice {
    fn cloned(&self) -> Box<WarehousesMicroservice>;
    fn with_superadmin(&self) -> Box<WarehousesMicroservice>;
    fn find_by_product_id(&self, product_id: ProductId) -> ApiFuture<Vec<Stock>>;
    fn set_product_in_warehouse(&self, warehouse_id: WarehouseId, product_id: ProductId, quantity: Quantity) -> ApiFuture<Stock>;
}

pub struct WarehousesMicroserviceImpl {
    http_client: Box<HttpClient>,
    config: config::Config,
}

impl WarehousesMicroservice for WarehousesMicroserviceImpl {
    fn cloned(&self) -> Box<WarehousesMicroservice> {
        Box::new(WarehousesMicroserviceImpl {
            http_client: self.http_client.cloned(),
            config: self.config.clone(),
        })
    }

    fn with_superadmin(&self) -> Box<WarehousesMicroservice> {
        Box::new(WarehousesMicroserviceImpl {
            http_client: self.http_client.superadmin(),
            config: self.config.clone(),
        })
    }

    fn set_product_in_warehouse(&self, warehouse_id: WarehouseId, product_id: ProductId, quantity: Quantity) -> ApiFuture<Stock> {
        let url = format!(
            "{}/warehouses/{}/products/{}",
            self.warehouses_url(),
            warehouse_identifier_route(&WarehouseIdentifier::Id(warehouse_id)),
            product_id
        );

        self.request::<StockSetPayload, Stock>(Method::Put, url, Some(StockSetPayload { quantity }), None)
    }

    fn find_by_product_id(&self, product_id: ProductId) -> ApiFuture<Vec<Stock>> {
        let url = format!("{}/stocks/by-product-id/{}", self.warehouses_url(), product_id);

        self.request::<(), Vec<Stock>>(Method::Get, url, None, None)
    }
}

impl WarehousesMicroserviceImpl {
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

use geo::Point as GeoPoint;

use stq_types::{Alpha3, StoreId, WarehouseId, WarehouseSlug};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Warehouse {
    pub id: WarehouseId,
    pub store_id: StoreId,
    pub slug: WarehouseSlug,
    pub name: Option<String>,
    pub location: Option<GeoPoint<f64>>,
    pub administrative_area_level_1: Option<String>,
    pub administrative_area_level_2: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<Alpha3>,
    pub locality: Option<String>,
    pub political: Option<String>,
    pub postal_code: Option<String>,
    pub route: Option<String>,
    pub street_number: Option<String>,
    pub address: Option<String>,
    pub place_id: Option<String>,
}

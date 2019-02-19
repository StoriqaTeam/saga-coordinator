use std::time::SystemTime;

use stq_static_resources::{Currency, ModerationStatus, Translation};
use stq_types::{BaseProductId, CategoryId, ProductId, ProductPrice, StoreId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoreModerate {
    pub store_id: StoreId,
    pub status: ModerationStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseProductModerate {
    pub base_product_id: BaseProductId,
    pub status: ModerationStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BaseProduct {
    pub id: BaseProductId,
    pub is_active: bool,
    pub store_id: StoreId,
    pub name: Vec<Translation>,
    pub short_description: Vec<Translation>,
    pub long_description: Option<Vec<Translation>>,
    pub seo_title: Option<Vec<Translation>>,
    pub seo_description: Option<Vec<Translation>>,
    pub currency: Currency,
    pub category_id: CategoryId,
    pub views: i32,
    pub rating: f64,
    pub slug: String,
    pub status: ModerationStatus,
    pub variants: Option<Vec<Product>>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub length_cm: Option<i32>,
    pub width_cm: Option<i32>,
    pub height_cm: Option<i32>,
    pub volume_cubic_cm: Option<i32>,
    pub weight_g: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub uuid: String,
    pub id: ProductId,
    pub base_product_id: BaseProductId,
    pub is_active: bool,
    pub discount: Option<f64>,
    pub photo_main: Option<String>,
    pub additional_photos: Option<Vec<String>>,
    pub vendor_code: String,
    pub cashback: Option<f64>,
    pub currency: Currency,
    pub price: ProductPrice,
    pub pre_order: bool,
    pub pre_order_days: i32,
    pub customer_price: CustomerPrice,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomerPrice {
    pub price: ProductPrice,
    pub currency: Currency,
}

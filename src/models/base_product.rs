use stq_static_resources::Currency;
use stq_types::{CategoryId, Quantity, StoreId};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UpdateBaseProduct {
    pub name: Option<serde_json::Value>,
    pub short_description: Option<serde_json::Value>,
    pub long_description: Option<serde_json::Value>,
    pub seo_title: Option<serde_json::Value>,
    pub seo_description: Option<serde_json::Value>,
    pub currency: Option<Currency>,
    pub category_id: Option<CategoryId>,
    pub slug: Option<String>,
    pub length_cm: Option<i32>,
    pub width_cm: Option<i32>,
    pub height_cm: Option<i32>,
    pub weight_g: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewBaseProductWithVariants {
    pub uuid: String,
    pub name: Vec<serde_json::Value>,
    pub store_id: StoreId,
    pub short_description: Vec<serde_json::Value>,
    pub long_description: Option<Vec<serde_json::Value>>,
    pub seo_title: Option<Vec<serde_json::Value>>,
    pub seo_description: Option<Vec<serde_json::Value>>,
    pub currency: Currency,
    pub category_id: i32,
    pub slug: Option<String>,
    pub variants: Vec<CreateProductWithAttributes>,
    pub selected_attributes: Vec<i32>,
    pub length_cm: Option<i32>,
    pub width_cm: Option<i32>,
    pub height_cm: Option<i32>,
    pub weight_g: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateProductWithAttributes {
    pub product: NewProduct,
    pub attributes: Vec<ProdAttrValue>,
    pub quantity: Option<Quantity>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewProduct {
    pub uuid: Option<String>,
    pub base_product_id: Option<i32>,
    pub discount: Option<f64>,
    pub photo_main: Option<String>,
    pub additional_photos: Option<Vec<String>>,
    pub vendor_code: String,
    pub cashback: Option<f64>,
    pub price: f64,
    pub pre_order: Option<bool>,
    pub pre_order_days: Option<i32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProdAttrValue {
    pub attr_id: i32,
    pub value: String,
    pub meta_field: Option<String>,
}

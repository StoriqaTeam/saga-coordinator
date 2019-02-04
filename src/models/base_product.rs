use stq_static_resources::Currency;
use stq_types::CategoryId;

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

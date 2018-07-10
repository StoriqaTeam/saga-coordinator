use std::time::SystemTime;

use serde_json;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub enum Status {
    Draft,
    Moderation,
    Decline,
    Published,
}

/// Payload for querying stores
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Store {
    pub id: i32,
    pub user_id: i32,
    pub is_active: bool,
    pub name: serde_json::Value,
    pub short_description: serde_json::Value,
    pub long_description: Option<serde_json::Value>,
    pub slug: String,
    pub cover: Option<String>,
    pub logo: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub facebook_url: Option<String>,
    pub twitter_url: Option<String>,
    pub instagram_url: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub default_language: String,
    pub slogan: Option<String>,
    pub rating: f64,
    pub country: Option<String>,
    pub product_categories: Option<serde_json::Value>,
    pub status: Status,
    pub administrative_area_level_1: Option<String>,
    pub administrative_area_level_2: Option<String>,
    pub locality: Option<String>,
    pub political: Option<String>,
    pub postal_code: Option<String>,
    pub route: Option<String>,
    pub street_number: Option<String>,
    pub place_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewStore {
    pub name: serde_json::Value,
    pub user_id: i32,
    pub short_description: serde_json::Value,
    pub long_description: Option<serde_json::Value>,
    pub slug: String,
    pub cover: Option<String>,
    pub logo: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub facebook_url: Option<String>,
    pub twitter_url: Option<String>,
    pub instagram_url: Option<String>,
    pub default_language: String,
    pub slogan: Option<String>,
    pub country: Option<String>,
    pub administrative_area_level_1: Option<String>,
    pub administrative_area_level_2: Option<String>,
    pub locality: Option<String>,
    pub political: Option<String>,
    pub postal_code: Option<String>,
    pub route: Option<String>,
    pub street_number: Option<String>,
    pub place_id: Option<String>,
    pub saga_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Payload {
    pub name: String,
    pub data: i32,
}

pub type NewRole = Payload;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Role {
    pub id: Uuid,
    pub user_id: i32,
    pub role: Payload,
}

#[derive(Clone, Copy, Debug, PartialEq, Hash, Serialize, Deserialize, Eq)]
pub struct StoreId(pub i32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStoreMerchantPayload {
    pub id: StoreId,
}

pub type CreateStoreOperationLog = Vec<CreateStoreOperationStage>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CreateStoreOperationStage {
    StoreCreationStart(i32),
    StoreCreationComplete(i32),
    WarehousesRoleSetStart(Uuid),
    WarehousesRoleSetComplete(Uuid),
    OrdersRoleSetStart(Uuid),
    OrdersRoleSetComplete(Uuid),
    BillingCreateMerchantStart(StoreId),
    BillingCreateMerchantComplete(StoreId),
}

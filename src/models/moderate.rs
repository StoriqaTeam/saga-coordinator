use stq_static_resources::ModerationStatus;
use stq_types::{BaseProductId, ProductId, StoreId};

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
    pub store_id: StoreId,
    pub slug: String,
    pub status: ModerationStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub id: ProductId,
}

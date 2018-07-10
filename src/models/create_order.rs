use std::collections::{BTreeMap, HashMap};

use chrono::prelude::*;
use uuid::Uuid;

use stq_static_resources::OrderStatus;

use super::*;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConvertCart {
    pub customer_id: UserId,
    #[serde(flatten)]
    pub address: Address,
    pub receiver_name: String,
    pub prices: CartProductWithPriceHash,
    pub currency_id: CurrencyId,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SagaId(pub Uuid);

impl SagaId {
    pub fn new() -> Self {
        SagaId(Uuid::new_v4())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub customer_id: UserId,
    pub saga_id: SagaId,
    pub currency_id: CurrencyId,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CurrencyId(pub i32);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Address {
    #[serde(rename = "address")]
    pub value: Option<String>,
    pub country: Option<String>,
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
pub struct Order {
    pub id: OrderId,
    pub state: OrderStatus,
    #[serde(rename = "customer")]
    pub customer_id: i32,
    #[serde(rename = "product")]
    pub product_id: i32,
    pub quantity: i32,
    #[serde(rename = "store")]
    pub store_id: i32,
    pub price: f64,
    pub receiver_name: String,
    pub slug: i32,
    pub payment_status: bool,
    pub delivery_company: Option<String>,
    pub track_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub address: Address,
}

pub type CartProductWithPriceHash = HashMap<i32, f64>;

pub type CreateOrderOperationLog = Vec<CreateOrderOperationStage>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillingOrders {
    pub orders: Vec<Order>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrdersCartItemInfo {
    pub quantity: i32,
    pub selected: bool,
    pub store_id: i32,
    pub comment: String,
}

pub type CartHash = BTreeMap<i32, OrdersCartItemInfo>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CreateOrderOperationStage {
    OrdersConvertCartStart(UserId),
    OrdersConvertCartComplete(UserId),
    BillingCreateInvoiceStart(SagaId),
    BillingCreateInvoiceComplete(SagaId),
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct OrderStatusPaid {
    state: OrderStatus,
    comment: Option<String>,
}

impl OrderStatusPaid {
    pub fn new() -> Self {
        Self {
            state: OrderStatus::Paid,
            comment: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderInfo {
    pub order_id: OrderId,
    pub customer_id: UserId,
    pub store_id: StoreId,
}

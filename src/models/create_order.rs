use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::SystemTime;

use chrono::prelude::*;

use stq_static_resources::OrderState;
use stq_types::{CurrencyId, InvoiceId, OrderId, ProductPrice, SagaId, StoreId, UserId};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConvertCart {
    pub customer_id: UserId,
    #[serde(flatten)]
    pub address: Address,
    pub receiver_name: String,
    pub prices: CartProductWithPriceHash,
    pub currency_id: CurrencyId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub customer_id: UserId,
    pub saga_id: SagaId,
    pub currency_id: CurrencyId,
}

impl fmt::Display for CreateInvoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CreateInvoice - orders: {:?}, customer_id: {}, saga_id: {}, currency_id: {})",
            self.orders, self.customer_id, self.saga_id, self.currency_id
        )
    }
}

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
    pub state: OrderState,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillingOrderInfo {
    pub order_id: OrderId,
    pub customer_id: UserId,
    pub store_id: StoreId,
    pub status: OrderState,
}

impl fmt::Display for BillingOrderInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BillingOrderInfo - order_id: {}, customer_id: {}, store_id: {}, status: {})",
            self.order_id, self.customer_id, self.store_id, self.status
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillingOrdersVec(pub Vec<BillingOrderInfo>);
impl fmt::Display for BillingOrdersVec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut comma_separated = String::new();

        for num in &self.0[0..self.0.len() - 1] {
            comma_separated.push_str(&num.to_string());
            comma_separated.push_str(", ");
        }

        comma_separated.push_str(&self.0[self.0.len() - 1].to_string());
        write!(f, "{}", comma_separated)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatePayload {
    pub state: OrderState,
    pub track_id: Option<String>,
    pub comment: Option<String>,
}

impl From<BillingOrderInfo> for UpdateStatePayload {
    fn from(order_info: BillingOrderInfo) -> Self {
        let comment = Some(format!("State changed to {} by billing service.", order_info.status).to_string());
        Self {
            state: order_info.status,
            track_id: None,
            comment,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResetMail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Invoice {
    pub invoice_id: InvoiceId,
    pub billing_url: String,
    pub transaction_id: Option<String>,
    pub transaction_captured_amount: Option<ProductPrice>,
    pub amount: ProductPrice,
    pub currency_id: CurrencyId,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: String,
}

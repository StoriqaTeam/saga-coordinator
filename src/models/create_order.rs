use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::SystemTime;

use uuid::Uuid;

use stq_api::orders::{AddressFull, CouponInfo, DeliveryInfo, Order, ProductInfo};
use stq_static_resources::{CommitterRole, Currency, CurrencyType, OrderState};
use stq_types::*;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConvertCart {
    pub customer_id: UserId,
    #[serde(flatten)]
    pub address: AddressFull,
    pub receiver_name: String,
    pub receiver_phone: String,
    pub receiver_email: String,
    pub prices: CartProductWithPriceHash,
    pub currency: Currency,
    pub coupons: HashMap<CouponId, CouponInfo>,
    pub delivery_info: HashMap<ProductId, DeliveryInfo>,
    pub product_info: HashMap<ProductId, ProductInfo>,
    pub uuid: Uuid,
    pub currency_type: Option<CurrencyType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BuyNow {
    pub product_id: ProductId,
    pub customer_id: UserId,
    pub store_id: StoreId,
    pub address: AddressFull,
    pub receiver_name: String,
    pub receiver_email: String,
    pub price: ProductSellerPrice,
    pub quantity: Quantity,
    pub currency: Currency,
    pub receiver_phone: String,
    pub pre_order: bool,
    pub pre_order_days: i32,
    pub coupon: Option<CouponInfo>,
    pub delivery_info: Option<DeliveryInfo>,
    pub product_info: ProductInfo,
    pub uuid: Uuid,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConvertCartWithConversionId {
    pub conversion_id: ConversionId,
    #[serde(flatten)]
    pub convert_cart: ConvertCart,
}

impl From<ConvertCart> for ConvertCartWithConversionId {
    fn from(convert_cart: ConvertCart) -> ConvertCartWithConversionId {
        ConvertCartWithConversionId {
            convert_cart,
            conversion_id: ConversionId::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ConvertCartRevert {
    pub conversion_id: ConversionId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateInvoice {
    pub orders: Vec<Order>,
    pub customer_id: UserId,
    pub saga_id: SagaId,
    pub currency: Currency,
}

impl fmt::Display for CreateInvoice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CreateInvoice - orders: {:?}, customer_id: {}, saga_id: {}, currency: {})",
            self.orders, self.customer_id, self.saga_id, self.currency
        )
    }
}

pub type CartProductWithPriceHash = HashMap<ProductId, ProductSellerPrice>;

pub type CreateOrderOperationLog = Vec<CreateOrderOperationStage>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BillingOrders {
    pub orders: Vec<Order>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrdersCartItemInfo {
    pub quantity: Quantity,
    pub selected: bool,
    pub store_id: StoreId,
    pub comment: String,
}

pub type CartHash = BTreeMap<i32, OrdersCartItemInfo>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CreateOrderOperationStage {
    OrdersConvertCartStart(ConversionId),
    OrdersConvertCartComplete(ConversionId),
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
        let comma_separated = self.0.iter().fold("".to_string(), |acc, i| format!("{}, {}", acc, i));
        write!(f, "{}", comma_separated)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatePayload {
    pub state: OrderState,
    pub track_id: Option<String>,
    pub comment: Option<String>,
    pub committer_role: CommitterRole,
}

impl From<BillingOrderInfo> for UpdateStatePayload {
    fn from(order_info: BillingOrderInfo) -> Self {
        let comment = Some(match order_info.status {
            OrderState::TransactionPending => "Found new transaction in blockchain, waiting for it confirmation.".to_string(),
            OrderState::AmountExpired => {
                "Invoice amount expiration timeout occurred, total amount will be recalculated by billing service.".to_string()
            }
            _ => format!("State changed to {} by billing service.", order_info.status).to_string(),
        });
        Self {
            state: order_info.status,
            track_id: None,
            comment,
            committer_role: CommitterRole::Customer,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Invoice {
    pub id: SagaId,
    pub invoice_id: InvoiceId,
    pub transactions: Vec<Transaction>,
    pub amount: ProductPrice,
    pub currency: Currency,
    pub price_reserved: SystemTime,
    pub state: OrderState,
    pub wallet: Option<String>,
    pub amount_captured: ProductPrice,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub amount_captured: ProductPrice,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UsedCoupon {
    pub coupon_id: CouponId,
    pub user_id: UserId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConvertCartPayload {
    pub conversion_id: Option<ConversionId>,
    pub user_id: UserId,
    pub receiver_name: String,
    pub receiver_phone: String,
    pub receiver_email: String,
    #[serde(flatten)]
    pub address: AddressFull,
    pub seller_prices: HashMap<ProductId, ProductSellerPrice>,
    pub coupons: HashMap<CouponId, CouponInfo>,
    pub delivery_info: HashMap<ProductId, DeliveryInfo>,
    pub product_info: HashMap<ProductId, ProductInfo>,
    pub uuid: Uuid,
    pub currency_type: Option<CurrencyType>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeleteProductsFromCartsPayload {
    pub product_ids: Vec<ProductId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeleteDeliveryMethodFromCartsPayload {
    pub product_ids: Vec<ProductId>,
}

impl From<ConvertCartWithConversionId> for ConvertCartPayload {
    fn from(other: ConvertCartWithConversionId) -> Self {
        let ConvertCartWithConversionId {
            conversion_id,
            convert_cart,
        } = other;

        Self {
            conversion_id: Some(conversion_id),
            user_id: convert_cart.customer_id,
            seller_prices: convert_cart.prices,
            address: convert_cart.address,
            receiver_name: convert_cart.receiver_name,
            receiver_phone: convert_cart.receiver_phone,
            receiver_email: convert_cart.receiver_email,
            coupons: convert_cart.coupons,
            delivery_info: convert_cart.delivery_info,
            product_info: convert_cart.product_info,
            uuid: convert_cart.uuid,
            currency_type: convert_cart.currency_type,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuyNowPayload {
    pub conversion_id: Option<ConversionId>,
    #[serde(flatten)]
    pub buy_now: BuyNow,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderPaymentStateRequest {
    pub state: PaymentState,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PaymentState {
    /// Order created and maybe paid by customer
    Initial,
    /// Store manager declined the order
    Declined,
    /// Store manager confirmed the order, money was captured
    Captured,
    /// Need money refund to customer
    RefundNeeded,
    /// Money was refunded to customer
    Refunded,
    /// Money was paid to seller
    PaidToSeller,
    /// Need money payment to seller
    PaymentToSellerNeeded,
}

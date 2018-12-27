use stq_types::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewShipping {
    pub items: Vec<NewProducts>,
    pub pickup: Option<NewPickups>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum ShippingVariant {
    Local,
    International,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Measurements {
    pub volume_cubic_cm: u32,
    pub weight_g: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewProducts {
    pub base_product_id: BaseProductId,
    pub store_id: StoreId,
    pub company_package_id: CompanyPackageId,
    pub price: Option<ProductPrice>,
    pub measurements: Measurements,
    pub delivery_from: Alpha3,
    pub deliveries_to: Vec<Alpha3>,
    pub shipping: ShippingVariant,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewPickups {
    pub base_product_id: BaseProductId,
    pub store_id: StoreId,
    pub pickup: bool,
    pub price: Option<ProductPrice>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Shipping {
    pub items: Vec<ShippingProducts>,
    pub pickup: Option<Pickups>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShippingProducts {
    pub product: Products,
    pub deliveries_to: Vec<Country>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Products {
    pub id: i32,
    pub base_product_id: BaseProductId,
    pub store_id: StoreId,
    pub company_package_id: CompanyPackageId,
    pub price: Option<ProductPrice>,
    pub deliveries_to: Vec<Alpha3>,
    pub shipping: ShippingVariant,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pickups {
    pub id: PickupId,
    pub base_product_id: BaseProductId,
    pub store_id: StoreId,
    pub pickup: bool,
    pub price: Option<ProductPrice>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Country {
    pub label: CountryLabel,
    pub parent: Option<Alpha3>,
    pub level: i32,
    pub alpha2: Alpha2,
    pub alpha3: Alpha3,
    pub numeric: i32,
    pub is_selected: bool,
    pub children: Vec<Country>,
}

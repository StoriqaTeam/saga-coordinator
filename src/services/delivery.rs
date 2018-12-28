use std::sync::Arc;

use failure::Error as FailureError;
use futures::future;
use futures::prelude::*;

use stq_types::*;

use super::parse_validation_errors;
use config;
use microservice::*;
use models::*;
use services::types::ServiceFuture;

pub trait DeliveryService {
    fn upsert_shipping(self, base_product_id: BaseProductId, payload: NewShipping) -> ServiceFuture<Box<DeliveryService>, Shipping>;
}

pub struct DeliveryServiceImpl {
    pub orders_microservice: Arc<OrdersMicroservice>,
    pub delivery_microservice: Arc<DeliveryMicroservice>,
    pub stores_microservice: Arc<StoresMicroservice>,
    pub config: config::Config,
}

impl DeliveryServiceImpl {
    pub fn new(
        config: config::Config,
        orders_microservice: Arc<OrdersMicroservice>,
        delivery_microservice: Arc<DeliveryMicroservice>,
        stores_microservice: Arc<StoresMicroservice>,
    ) -> Self {
        Self {
            config,
            orders_microservice,
            delivery_microservice,
            stores_microservice,
        }
    }

    fn remove_products_from_cart_after_shipping_change(
        self,
        base_product_id: BaseProductId,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let stores_microservice = self.stores_microservice.clone();
        let orders_microservice = self.orders_microservice.clone();
        let fut = stores_microservice
            .get_products_by_base_product(base_product_id)
            .map(|products| DeleteDeliveryMethodFromCartsPayload {
                product_ids: products.into_iter().map(|p| p.id).collect(),
            })
            .and_then(move |payload| orders_microservice.delete_delivery_method_from_all_carts(Some(Initiator::Superadmin), payload));

        let res = Box::new(fut);

        res.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(err) => Err((self, err)),
        })
    }
}

impl DeliveryService for DeliveryServiceImpl {
    fn upsert_shipping(self, base_product_id: BaseProductId, payload: NewShipping) -> ServiceFuture<Box<DeliveryService>, Shipping> {
        debug!("Update shipping, input: {:?} for base product: {:?}", payload, base_product_id);

        let res = self
            .delivery_microservice
            .upsert_shipping(None, base_product_id, payload)
            .then(|res| match res {
                Ok(shipping) => Ok((self, shipping)),
                Err(e) => Err((self, e)),
            })
            .and_then(move |(s, shipping)| {
                s.remove_products_from_cart_after_shipping_change(base_product_id)
                    .map(|(s, _)| (s, shipping))
            })
            .map(|(s, shipping)| (Box::new(s) as Box<DeliveryService>, shipping))
            .or_else(|(s, e)| future::err((Box::new(s) as Box<DeliveryService>, parse_validation_errors(e, &["shipping"]))));

        Box::new(res)
    }
}

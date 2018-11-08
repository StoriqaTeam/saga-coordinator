use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures;
use futures::future;
use futures::prelude::*;
use futures::stream::iter_ok;
use hyper::header::Authorization;
use hyper::Headers;

use stq_types::{BillingRole, DeliveryRole, OrderRole, RoleEntryId, RoleId, StoreId, UserId, WarehouseRole};

use super::parse_validation_errors;
use config;
use errors::Error;
use microservice::*;
use models::*;
use services::types::ServiceFuture;

pub trait StoreService {
    fn create(self, input: NewStore) -> ServiceFuture<Box<StoreService>, Option<Store>>;
}

/// Attributes services, responsible for Attribute-related CRUD operations
pub struct StoreServiceImpl {
    pub orders_microservice: Arc<OrdersMicroservice>,
    pub stores_microservice: Arc<StoresMicroservice>,
    pub billing_microservice: Arc<BillingMicroservice>,
    pub warehouses_microservice: Arc<WarehousesMicroservice>,
    pub delivery_microservice: Arc<DeliveryMicroservice>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateStoreOperationLog>>,
}

impl StoreServiceImpl {
    pub fn new(
        config: config::Config,
        orders_microservice: Arc<OrdersMicroservice>,
        stores_microservice: Arc<StoresMicroservice>,
        billing_microservice: Arc<BillingMicroservice>,
        warehouses_microservice: Arc<WarehousesMicroservice>,
        delivery_microservice: Arc<DeliveryMicroservice>,
    ) -> Self {
        let log = Arc::new(Mutex::new(CreateStoreOperationLog::new()));
        Self {
            config,
            log,
            orders_microservice,
            stores_microservice,
            billing_microservice,
            warehouses_microservice,
            delivery_microservice,
        }
    }

    fn create_store(self, input: &NewStore) -> ServiceFuture<Self, Store> {
        // Create Store
        debug!("Creating store, input: {:?}", input);

        let log = self.log.clone();
        let user_id = input.user_id;
        log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationStart(user_id));

        let res = self
            .stores_microservice
            .create_store(None, input.clone())
            .map_err(|e| {
                e.context("Creating store in stores microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |store| {
                log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationComplete(store.id));
                Ok(store)
            }).then(|res| match res {
                Ok(store) => Ok((self, store)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_warehouses_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, RoleEntry<NewWarehouseRole>> {
        // Create warehouses role
        debug!("Creating warehouses role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role_payload = NewWarehouseRole {
            name: WarehouseRole::StoreManager,
            data: store_id,
        };
        let role = RoleEntry::<NewWarehouseRole>::new(new_role_id, user_id, role_payload);

        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::WarehousesRoleSetStart(new_role_id));

        let res = self
            .warehouses_microservice
            .create_warehouse_role(Some(Initiator::Superadmin), role)
            .map_err(|e| {
                e.context("Creating role in warehouses microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::WarehousesRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(warehouses_role) => Ok((self, warehouses_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_orders_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, RoleEntry<NewOrdersRole>> {
        // Create orders role
        debug!("Creating orders role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role_payload = NewOrdersRole {
            name: OrderRole::StoreManager,
            data: store_id,
        };
        let role = RoleEntry::<NewOrdersRole>::new(new_role_id, user_id, role_payload);

        log.lock().unwrap().push(CreateStoreOperationStage::OrdersRoleSetStart(new_role_id));

        let res = self
            .orders_microservice
            .create_role(Some(Initiator::Superadmin), role.clone())
            .map_err(|e| {
                e.context("Creating role in orders microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::OrdersRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(orders_role) => Ok((self, orders_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_billing_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, NewRole<BillingRole>> {
        // Create billing role
        debug!("Creating billing role, user id: {}", user_id);
        let log = self.log.clone();

        let new_role_id = RoleId::new();
        let role = NewRole::<BillingRole>::new(new_role_id, user_id, BillingRole::StoreManager, Some(store_id));

        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::BillingRoleSetStart(new_role_id));

        let res = self
            .billing_microservice
            .create_role(Some(Initiator::Superadmin), role)
            .map_err(|e| {
                e.context("Creating role in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::BillingRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(billing_role) => Ok((self, billing_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_delivery_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, NewRole<DeliveryRole>> {
        // Create delivery role
        debug!("Creating delivery role, user id: {}", user_id);
        let log = self.log.clone();

        let new_role_id = RoleId::new();
        let role = NewRole::<DeliveryRole>::new(new_role_id, user_id, DeliveryRole::StoreManager, Some(store_id));

        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::DeliveryRoleSetStart(new_role_id));

        let res = self
            .delivery_microservice
            .create_delivery_role(Some(Initiator::Superadmin), role)
            .map_err(|e| {
                e.context("Creating role in delivery microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::DeliveryRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(delivery_role) => Ok((self, delivery_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_merchant(self, store_id: StoreId) -> ServiceFuture<Self, Merchant> {
        debug!("Creating merchant for store_id: {}", store_id);
        let payload = CreateStoreMerchantPayload { id: store_id };

        // Create store role
        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::BillingCreateMerchantStart(store_id));

        let res = self
            .billing_microservice
            .create_store_merchant(Some(Initiator::Superadmin), payload)
            .map_err(|e| {
                e.context("Creating merchant in billing microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::BillingCreateMerchantComplete(store_id));
                Ok(res)
            }).then(|res| match res {
                Ok(merchant) => Ok((self, merchant)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    // Contains happy path for Store creation
    fn create_happy(self, input: &NewStore) -> ServiceFuture<Self, Store> {
        Box::new(
            self.create_store(&input)
                .and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_warehouses_role(user_id, store_id).map(|(s, _)| (s, store))
                }).and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_orders_role(user_id, store_id).map(|(s, _)| (s, store))
                }).and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_billing_role(user_id, store_id).map(|(s, _)| (s, store))
                }).and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_delivery_role(user_id, store_id).map(|(s, _)| (s, store))
                }).and_then(|(s, store)| s.create_merchant(store.id).map(|(s, _)| (s, store))),
        )
    }

    // Contains reversal of Store creation
    fn create_revert(self) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let log = self.log.lock().unwrap().clone();

        let orders_microservice = self.orders_microservice.clone();
        let stores_microservice = self.stores_microservice.clone();
        let billing_microservice = self.billing_microservice.clone();
        let warehouses_microservice = self.warehouses_microservice.clone();
        let delivery_microservice = self.delivery_microservice.clone();
        let fut = iter_ok::<_, ()>(log).for_each(move |e| {
            match e {
                CreateStoreOperationStage::StoreCreationComplete(store_id) => {
                    debug!("Reverting store, store_id: {}", store_id);
                    Box::new(
                        stores_microservice
                            .delete_store(Some(Initiator::Superadmin), store_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateStoreOperationStage::WarehousesRoleSetComplete(role_id) => {
                    debug!("Reverting warehouses role, user_id: {}", role_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete user role

                    Box::new(
                        warehouses_microservice
                            .delete_warehouse_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateStoreOperationStage::OrdersRoleSetComplete(role_id) => {
                    debug!("Reverting orders role, user_id: {}", role_id);
                    Box::new(
                        orders_microservice
                            .delete_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateStoreOperationStage::BillingRoleSetComplete(role_id) => {
                    debug!("Reverting billing role, user_id: {}", role_id);

                    Box::new(
                        billing_microservice
                            .delete_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateStoreOperationStage::DeliveryRoleSetComplete(role_id) => {
                    debug!("Reverting delivery role, role_id: {}", role_id);
                    Box::new(
                        delivery_microservice
                            .delete_delivery_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateStoreOperationStage::BillingCreateMerchantComplete(store_id) => {
                    debug!("Reverting merchant, store_id: {}", store_id);

                    Box::new(
                        billing_microservice
                            .delete_store_merchant(Some(Initiator::Superadmin), store_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                _ => Box::new(future::ok(())) as Box<Future<Item = (), Error = ()>>,
            }
        });

        fut.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(_) => Err((self, format_err!("Order service create_revert error occured."))),
        })
    }
}

impl StoreService for StoreServiceImpl {
    fn create(self, input: NewStore) -> ServiceFuture<Box<StoreService>, Option<Store>> {
        Box::new(
            self.create_happy(&input)
                .map(|(s, store)| (Box::new(s) as Box<StoreService>, Some(store)))
                .or_else(move |(s, e)| {
                    s.create_revert().then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        futures::future::err((Box::new(s) as Box<StoreService>, e))
                    })
                }).map_err(|(s, e): (Box<StoreService>, FailureError)| {
                    (
                        s,
                        parse_validation_errors(
                            e,
                            &[
                                "name",
                                "short_description",
                                "long_description",
                                "slug",
                                "phone",
                                "email",
                                "default_language",
                                "store",
                            ],
                        ),
                    )
                }),
        )
    }
}

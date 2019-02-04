use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures;
use futures::future::{self, Either};
use futures::prelude::*;
use futures::stream::iter_ok;
use hyper::header::Authorization;
use hyper::Headers;

use stq_types::{BaseProductId, BillingRole, DeliveryRole, OrderRole, ProductId, RoleEntryId, RoleId, StoreId, UserId, WarehouseRole};

use stq_static_resources::{
    BaseProductModerationStatusForModerator, BaseProductModerationStatusForUser, EmailUser, ModerationStatus,
    StoreModerationStatusForModerator, StoreModerationStatusForUser,
};

use super::parse_validation_errors;
use config;
use errors::Error;
use microservice::*;
use models::*;
use services::types::ServiceFuture;

pub trait StoreService {
    fn create(self, input: NewStore) -> ServiceFuture<Box<StoreService>, Option<Store>>;
    /// Set moderation status for specific store
    fn set_store_moderation_status(self, payload: StoreModerate) -> ServiceFuture<Box<StoreService>, Store>;
    /// Send store to moderation from store manager
    fn send_to_moderation(self, store_id: StoreId) -> ServiceFuture<Box<StoreService>, Store>;
    /// Set moderation status for base_product_id
    fn set_moderation_status_base_product(self, payload: BaseProductModerate) -> ServiceFuture<Box<StoreService>, ()>;
    /// send base product to moderation from store manager
    fn send_to_moderation_base_product(self, base_product_id: BaseProductId) -> ServiceFuture<Box<StoreService>, ()>;
    /// Deactivate base product
    fn deactivate_base_product(self, base_product_id: BaseProductId) -> ServiceFuture<Box<StoreService>, BaseProduct>;
    /// Deactivate store
    fn deactivate_store(self, store: StoreId) -> ServiceFuture<Box<StoreService>, Store>;
    /// Deactivate product
    fn deactivate_product(self, product_id: ProductId) -> ServiceFuture<Box<StoreService>, Product>;
    /// Update base product
    fn update_base_product(
        self,
        base_product_id: BaseProductId,
        payload: UpdateBaseProduct,
    ) -> ServiceFuture<Box<StoreService>, BaseProduct>;
}

pub struct StoreServiceImpl {
    pub orders_microservice: Arc<OrdersMicroservice>,
    pub stores_microservice: Arc<StoresMicroservice>,
    pub notifications_microservice: Arc<NotificationsMicroservice>,
    pub billing_microservice: Arc<BillingMicroservice>,
    pub warehouses_microservice: Arc<WarehousesMicroservice>,
    pub delivery_microservice: Arc<DeliveryMicroservice>,
    pub users_microservice: Arc<UsersMicroservice>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateStoreOperationLog>>,
}

impl StoreServiceImpl {
    pub fn new(
        config: config::Config,
        orders_microservice: Arc<OrdersMicroservice>,
        stores_microservice: Arc<StoresMicroservice>,
        notifications_microservice: Arc<NotificationsMicroservice>,
        billing_microservice: Arc<BillingMicroservice>,
        warehouses_microservice: Arc<WarehousesMicroservice>,
        users_microservice: Arc<UsersMicroservice>,
        delivery_microservice: Arc<DeliveryMicroservice>,
    ) -> Self {
        let log = Arc::new(Mutex::new(CreateStoreOperationLog::new()));
        Self {
            config,
            log,
            orders_microservice,
            stores_microservice,
            notifications_microservice,
            billing_microservice,
            warehouses_microservice,
            users_microservice,
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
            .and_then(move |store| {
                log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationComplete(store.id));
                Ok(store)
            })
            .then(|res| match res {
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
            .and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::WarehousesRoleSetComplete(new_role_id));
                Ok(res)
            })
            .then(|res| match res {
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
            .and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::OrdersRoleSetComplete(new_role_id));
                Ok(res)
            })
            .then(|res| match res {
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
            .and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::BillingRoleSetComplete(new_role_id));
                Ok(res)
            })
            .then(|res| match res {
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
            })
            .and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::DeliveryRoleSetComplete(new_role_id));
                Ok(res)
            })
            .then(|res| match res {
                Ok(delivery_role) => Ok((self, delivery_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_merchant(self, store_id: StoreId, store_country_code: Option<String>) -> ServiceFuture<Self, Merchant> {
        debug!("Creating merchant for store_id: {}", store_id);
        let payload = CreateStoreMerchantPayload {
            id: store_id,
            country_code: store_country_code,
        };

        // Create store role
        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::BillingCreateMerchantStart(store_id));

        let res = self
            .billing_microservice
            .create_store_merchant(Some(Initiator::Superadmin), payload)
            .and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::BillingCreateMerchantComplete(store_id));
                Ok(res)
            })
            .then(|res| match res {
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
                })
                .and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_orders_role(user_id, store_id).map(|(s, _)| (s, store))
                })
                .and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_billing_role(user_id, store_id).map(|(s, _)| (s, store))
                })
                .and_then(|(s, store)| {
                    let user_id = store.user_id;
                    let store_id = store.id;
                    s.create_delivery_role(user_id, store_id).map(|(s, _)| (s, store))
                })
                .and_then(|(s, store)| s.create_merchant(store.id, store.country_code.clone()).map(|(s, _)| (s, store))),
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

    fn set_store_moderation_status(self, payload: StoreModerate) -> ServiceFuture<Self, Store> {
        let res = self.stores_microservice.set_store_moderation_status(payload).then(|res| match res {
            Ok(store) => Ok((self, store)),
            Err(_) => Err((self, format_err!("Store service set_moderation_status error occurred."))),
        });

        Box::new(res)
    }

    fn send_to_moderation(self, store_id: StoreId) -> ServiceFuture<Self, Store> {
        let res = self.stores_microservice.send_to_moderation(store_id).then(|res| match res {
            Ok(store) => Ok((self, store)),
            Err(_) => Err((self, format_err!("Store service send_to_moderation error occurred."))),
        });

        Box::new(res)
    }

    fn set_moderation_status_base_product(self, payload: BaseProductModerate) -> ServiceFuture<Self, BaseProduct> {
        let res = self
            .stores_microservice
            .set_moderation_status_base_product(payload)
            .then(|res| match res {
                Ok(base) => Ok((self, base)),
                Err(_) => Err((
                    self,
                    format_err!("Store service set_moderation_status_base_product error occurred."),
                )),
            });

        Box::new(res)
    }

    fn send_to_moderation_base_product(self, base_product_id: BaseProductId) -> ServiceFuture<Self, BaseProduct> {
        let res = self
            .stores_microservice
            .send_to_moderation_base_product(base_product_id)
            .then(|res| match res {
                Ok(base) => Ok((self, base)),
                Err(_) => Err((self, format_err!("Store service send_to_moderation_base_product error occurred."))),
            });

        Box::new(res)
    }

    fn notify_moderators_base_product_update_moderation_status(
        self,
        store_id: StoreId,
        base_product_id: BaseProductId,
        status: ModerationStatus,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        info!("get moderators from stores microservice");

        let stores_microservice = self.stores_microservice.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let users_microservice = self.users_microservice.clone();
        let cluster_url = self.config.cluster.url.clone();

        stores_microservice
            .get_moderators(Initiator::Superadmin)
            .map_err(FailureError::from)
            .and_then(move |results| {
                let fut = iter_ok::<_, FailureError>(results).for_each(move |moderator_id| {
                    let notif = notifications_microservice.clone();
                    let cluster_url = cluster_url.clone();

                    Box::new(
                        users_microservice
                            .clone()
                            .get(Some(Initiator::Superadmin), moderator_id)
                            .and_then(move |moderator| {
                                if let Some(user) = moderator {
                                    let email_user = EmailUser {
                                        email: user.email.clone(),
                                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                                    };
                                    let email = BaseProductModerationStatusForModerator {
                                        user: email_user,
                                        store_id: store_id.to_string(),
                                        base_product_id: base_product_id.to_string(),
                                        cluster_url,
                                        status,
                                    };
                                    Either::A(
                                        notif
                                            .base_product_moderation_status_for_moderator(Initiator::Superadmin, email)
                                            .then(|_| Ok(())),
                                    )
                                } else {
                                    Either::B(future::ok(()))
                                }
                            }),
                    ) as Box<Future<Item = (), Error = FailureError>>
                });

                fut
            })
            .then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(e) => Err((self, e)),
            })
    }

    fn notify_manager_store_update_moderation_status(
        self,
        store_id: StoreId,
        store_manager_id: UserId,
        status: ModerationStatus,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let cluster_url = self.config.cluster.url.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let users_microservice = self.users_microservice.clone();

        let fut = Box::new(
            users_microservice
                .get(Some(Initiator::Superadmin), store_manager_id)
                .and_then(move |store_manager| {
                    if let Some(user) = store_manager {
                        let email = StoreModerationStatusForUser {
                            store_email: user.email.to_string(),
                            store_id: store_id.to_string(),
                            cluster_url,
                            status,
                        };

                        Either::A(
                            notifications_microservice
                                .store_moderation_status_for_user(Initiator::Superadmin, email)
                                .then(|_| Ok(())),
                        )
                    } else {
                        Either::B(future::ok(()))
                    }
                }),
        ) as Box<Future<Item = (), Error = FailureError>>;

        fut.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(e) => Err((self, e)),
        })
    }

    fn notify_manager_base_product_update_moderation_status(
        self,
        store_id: StoreId,
        base_product_id: BaseProductId,
        status: ModerationStatus,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let cluster_url = self.config.cluster.url.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let users_microservice = self.users_microservice.clone();
        let stores_microservice = self.stores_microservice.clone();

        let fut = Box::new(
            stores_microservice
                .get(store_id, Visibility::Active)
                .and_then(move |store| {
                    store
                        .ok_or_else(|| {
                            error!(
                                "Sending notification to store can not be done. Store with id: {} is not found.",
                                store_id
                            );
                            format_err!("Store is not found in stores microservice.")
                                .context(Error::NotFound)
                                .into()
                        })
                        .into_future()
                })
                .and_then(move |store| {
                    users_microservice
                        .get(Some(Initiator::Superadmin), store.user_id)
                        .and_then(move |store_manager| {
                            if let Some(user) = store_manager {
                                let email = BaseProductModerationStatusForUser {
                                    store_email: user.email.to_string(),
                                    store_id: store_id.to_string(),
                                    base_product_id: base_product_id.to_string(),
                                    cluster_url,
                                    status,
                                };

                                Either::A(
                                    notifications_microservice
                                        .base_product_moderation_status_for_user(Initiator::Superadmin, email)
                                        .then(|_| Ok(())),
                                )
                            } else {
                                Either::B(future::ok(()))
                            }
                        })
                }),
        ) as Box<Future<Item = (), Error = FailureError>>;

        fut.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(e) => Err((self, e)),
        })
    }

    fn notify_moderators_store_update_moderation_status(
        self,
        store_id: StoreId,
        status: ModerationStatus,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        info!("get moderators from stores microservice");

        let stores_microservice = self.stores_microservice.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let users_microservice = self.users_microservice.clone();
        let cluster_url = self.config.cluster.url.clone();

        stores_microservice
            .get_moderators(Initiator::Superadmin)
            .map_err(FailureError::from)
            .and_then(move |results| {
                let fut = iter_ok::<_, FailureError>(results).for_each(move |moderator_id| {
                    let notif = notifications_microservice.clone();
                    let cluster_url = cluster_url.clone();

                    Box::new(
                        users_microservice
                            .clone()
                            .get(Some(Initiator::Superadmin), moderator_id)
                            .and_then(move |moderator| {
                                if let Some(user) = moderator {
                                    let email_user = EmailUser {
                                        email: user.email.clone(),
                                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                                    };
                                    let email = StoreModerationStatusForModerator {
                                        user: email_user,
                                        store_id: store_id.to_string(),
                                        cluster_url,
                                        status,
                                    };
                                    Either::A(
                                        notif
                                            .store_moderation_status_for_moderator(Initiator::Superadmin, email)
                                            .then(|_| Ok(())),
                                    )
                                } else {
                                    Either::B(future::ok(()))
                                }
                            }),
                    ) as Box<Future<Item = (), Error = FailureError>>
                });

                fut
            })
            .then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(e) => Err((self, e)),
            })
    }

    fn remove_products_from_cart_after_base_product_status_change(
        self,
        base_product_id: BaseProductId,
        initial_status: ModerationStatus,
        status: ModerationStatus,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let stores_microservice = self.stores_microservice.clone();
        let orders_microservice = self.orders_microservice.clone();
        let res: Box<Future<Item = (), Error = FailureError>> = if is_status_change_requires_to_delete_product(initial_status, status) {
            let fut = stores_microservice
                .get_products_by_base_product(base_product_id)
                .map(|products| DeleteProductsFromCartsPayload {
                    product_ids: products.into_iter().map(|p| p.id).collect(),
                })
                .and_then(move |payload| orders_microservice.delete_products_from_all_carts(Some(Initiator::Superadmin), payload));
            Box::new(fut)
        } else {
            //do nothing
            Box::new(Ok(()).into_future())
        };
        res.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(err) => Err((self, err)),
        })
    }

    fn remove_products_from_cart_after_store_status_change(
        self,
        store_id: StoreId,
        initial_status: ModerationStatus,
        status: ModerationStatus,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let stores_microservice = self.stores_microservice.clone();
        let orders_microservice = self.orders_microservice.clone();
        let res: Box<Future<Item = (), Error = FailureError>> = if is_status_change_requires_to_delete_product(initial_status, status) {
            let fut = stores_microservice
                .get_products_by_store(store_id)
                .map(|products| DeleteProductsFromCartsPayload {
                    product_ids: products.into_iter().map(|p| p.id).collect(),
                })
                .and_then(move |payload| orders_microservice.delete_products_from_all_carts(Some(Initiator::Superadmin), payload));
            Box::new(fut)
        } else {
            //do nothing
            Box::new(Ok(()).into_future())
        };
        res.then(|res| match res {
            Ok(_) => Ok((self, ())),
            Err(err) => Err((self, err)),
        })
    }

    fn remove_products_from_cart_after_base_product_deactivation(
        self,
        base_product_id: BaseProductId,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let stores_microservice = self.stores_microservice.clone();
        let orders_microservice = self.orders_microservice.clone();
        stores_microservice
            .get_products_by_base_product(base_product_id)
            .map(|products| DeleteProductsFromCartsPayload {
                product_ids: products.into_iter().map(|p| p.id).collect(),
            })
            .and_then(move |payload| orders_microservice.delete_products_from_all_carts(Some(Initiator::Superadmin), payload))
            .then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(err) => Err((self, err)),
            })
    }

    fn remove_products_from_cart_after_store_deactivation(
        self,
        store_id: StoreId,
    ) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let stores_microservice = self.stores_microservice.clone();
        let orders_microservice = self.orders_microservice.clone();
        stores_microservice
            .get_products_by_store(store_id)
            .map(|products| DeleteProductsFromCartsPayload {
                product_ids: products.into_iter().map(|p| p.id).collect(),
            })
            .and_then(move |payload| orders_microservice.delete_products_from_all_carts(Some(Initiator::Superadmin), payload))
            .then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(err) => Err((self, err)),
            })
    }

    fn after_base_product_update(self, base_product_id: BaseProductId) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let stores_microservice = self.stores_microservice.clone();
        let orders_microservice = self.orders_microservice.clone();
        stores_microservice
            .get_products_by_base_product(base_product_id)
            .map(|products| DeleteProductsFromCartsPayload {
                product_ids: products.into_iter().map(|p| p.id).collect(),
            })
            .and_then(move |payload| orders_microservice.delete_products_from_all_carts(Some(Initiator::Superadmin), payload))
            .then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(err) => Err((self, err)),
            })
    }
}

fn is_status_change_requires_to_delete_product(initial_status: ModerationStatus, status: ModerationStatus) -> bool {
    match (initial_status, status) {
        (ModerationStatus::Published, status) if status != ModerationStatus::Published => true,
        _ => false,
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
                })
                .map_err(|(s, e): (Box<StoreService>, FailureError)| {
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

    fn set_store_moderation_status(self, payload: StoreModerate) -> ServiceFuture<Box<StoreService>, Store> {
        Box::new(
            self.stores_microservice
                .get(payload.store_id, Visibility::Active)
                .then(|res| match res {
                    Ok(Some(store)) => Ok((self, store.status)),
                    Ok(None) => Err((
                        self,
                        format_err!("Store is not found in stores microservice.")
                            .context(Error::NotFound)
                            .into(),
                    )),
                    Err(err) => Err((self, err)),
                })
                .and_then(|(s, initial_status)| {
                    s.set_store_moderation_status(payload)
                        .map(move |(s, store)| (s, store, initial_status))
                })
                .and_then(|(s, store, initial_status)| {
                    s.remove_products_from_cart_after_store_status_change(store.id, initial_status, store.status)
                        .map(|(s, _)| (s, store))
                })
                .and_then(|(s, store)| {
                    s.notify_manager_store_update_moderation_status(store.id, store.user_id, store.status)
                        .map(|(s, _)| (s, store))
                })
                .map(|(s, store)| (Box::new(s) as Box<StoreService>, store))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Send store to moderation from store manager
    fn send_to_moderation(self, store_id: StoreId) -> ServiceFuture<Box<StoreService>, Store> {
        Box::new(
            self.send_to_moderation(store_id)
                .and_then(|(s, store)| {
                    s.notify_moderators_store_update_moderation_status(store.id, store.status)
                        .map(|(s, _)| (s, store))
                })
                .map(|(s, store)| (Box::new(s) as Box<StoreService>, store))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Set moderation status for base_product_id
    fn set_moderation_status_base_product(self, payload: BaseProductModerate) -> ServiceFuture<Box<StoreService>, ()> {
        Box::new(
            self.stores_microservice
                .get_base_product(payload.base_product_id, Visibility::Active)
                .then(move |res| match res {
                    Ok(Some(base_product)) => Ok((self, base_product.status)),
                    Ok(None) => Err((
                        self,
                        format_err!("Base product is not found in stores microservice.")
                            .context(Error::NotFound)
                            .into(),
                    )),
                    Err(err) => Err((self, err)),
                })
                .and_then(|(s, initial_status)| {
                    s.set_moderation_status_base_product(payload)
                        .map(move |(s, base_product)| (s, initial_status, base_product))
                })
                .and_then(|(s, initial_status, base_product)| {
                    s.remove_products_from_cart_after_base_product_status_change(base_product.id, initial_status, base_product.status)
                        .map(|(s, _)| (s, base_product))
                })
                .and_then(|(s, base)| {
                    s.notify_manager_base_product_update_moderation_status(base.store_id, base.id, base.status)
                        .map(|(s, _)| (s, ()))
                })
                .map(|(s, _)| (Box::new(s) as Box<StoreService>, ()))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Send base product to moderation from store manager
    fn send_to_moderation_base_product(self, base_product_id: BaseProductId) -> ServiceFuture<Box<StoreService>, ()> {
        Box::new(
            self.send_to_moderation_base_product(base_product_id)
                .and_then(|(s, base)| {
                    s.notify_moderators_base_product_update_moderation_status(base.store_id, base.id, base.status)
                        .map(|(s, _)| (s, ()))
                })
                .map(|(s, _)| (Box::new(s) as Box<StoreService>, ()))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Deactivate base product
    fn deactivate_base_product(self, base_product_id: BaseProductId) -> ServiceFuture<Box<StoreService>, BaseProduct> {
        Box::new(
            self.stores_microservice
                .deactivate_base_product(None, base_product_id)
                .then(move |res| match res {
                    Ok(base_product) => Ok((self, base_product)),
                    Err(err) => Err((self, err)),
                })
                .and_then(move |(s, base_product)| {
                    s.remove_products_from_cart_after_base_product_deactivation(base_product_id)
                        .map(move |(s, _)| (s, base_product))
                })
                .map(|(s, base_product)| (Box::new(s) as Box<StoreService>, base_product))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Deactivate store
    fn deactivate_store(self, store_id: StoreId) -> ServiceFuture<Box<StoreService>, Store> {
        Box::new(
            self.stores_microservice
                .deactivate_store(None, store_id)
                .then(move |res| match res {
                    Ok(store) => Ok((self, store)),
                    Err(err) => Err((self, err)),
                })
                .and_then(move |(s, store)| {
                    s.remove_products_from_cart_after_store_deactivation(store_id)
                        .map(move |(s, _)| (s, store))
                })
                .map(|(s, store)| (Box::new(s) as Box<StoreService>, store))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Deactivate product
    fn deactivate_product(self, product_id: ProductId) -> ServiceFuture<Box<StoreService>, Product> {
        let orders_microservice = self.orders_microservice.clone();
        Box::new(
            self.stores_microservice
                .deactivate_product(None, product_id)
                .then(move |res| match res {
                    Ok(product) => Ok((self, product)),
                    Err(err) => Err((self, err)),
                })
                .and_then(move |(s, product)| {
                    orders_microservice
                        .delete_products_from_all_carts(
                            Some(Initiator::Superadmin),
                            DeleteProductsFromCartsPayload {
                                product_ids: vec![product_id],
                            },
                        )
                        .then(move |res| match res {
                            Ok(_) => Ok((s, product)),
                            Err(err) => Err((s, err)),
                        })
                })
                .map(|(s, product)| (Box::new(s) as Box<StoreService>, product))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }

    /// Update base product
    fn update_base_product(
        self,
        base_product_id: BaseProductId,
        payload: UpdateBaseProduct,
    ) -> ServiceFuture<Box<StoreService>, BaseProduct> {
        Box::new(
            self.stores_microservice
                .update_base_product(None, base_product_id, payload)
                .then(move |res| match res {
                    Ok(base_product) => Ok((self, base_product)),
                    Err(err) => Err((self, err)),
                })
                .and_then(move |(s, base_product)| s.after_base_product_update(base_product_id).map(|(s, _)| (s, base_product)))
                .map(|(s, base_product)| (Box::new(s) as Box<StoreService>, base_product))
                .or_else(|(s, e)| future::err((Box::new(s) as Box<StoreService>, e))),
        )
    }
}

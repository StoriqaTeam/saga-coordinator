use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use futures;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;

use serde_json;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::{MerchantId, RoleEntryId, StoreId, StoresRole, UserId};

use super::parse_validation_errors;
use config;
use errors::Error;
use models::create_store::Role;
use models::*;
use services::types::ServiceFuture;

pub trait StoreService {
    fn create(self, input: NewStore) -> ServiceFuture<Box<StoreService>, Option<Store>>;
}

/// Attributes services, responsible for Attribute-related CRUD operations
pub struct StoreServiceImpl {
    pub http_client: Arc<HttpClientHandle>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateStoreOperationLog>>,
    pub user_id: Option<i32>,
}

impl StoreServiceImpl {
    pub fn new(http_client: Arc<HttpClientHandle>, config: config::Config, user_id: Option<i32>) -> Self {
        let log = Arc::new(Mutex::new(CreateStoreOperationLog::new()));
        Self {
            http_client,
            config,
            log,
            user_id,
        }
    }

    fn create_store(self, input: &NewStore) -> ServiceFuture<Self, Store> {
        // Create Store
        debug!("Creating store, input: {:?}", input);

        let log = self.log.clone();
        let user_id = input.user_id;
        log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationStart(user_id));

        let mut headers = Headers::new();
        if let Some(ref user_id) = self.user_id {
            headers.set(Authorization(user_id.to_string()));
        };

        let client = self.http_client.clone();
        let stores_url = self.config.service_url(StqService::Stores);

        let res = serde_json::to_string(input)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<Store>(
                        Method::Post,
                        format!("{}/{}", stores_url, StqModel::Store.to_url()),
                        Some(body),
                        Some(headers),
                    )
                    .map_err(|e| {
                        format_err!("Creating store in stores microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationComplete(user_id));
            })
            .then(|res| match res {
                Ok(store) => Ok((self, store)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_warehouse_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, Role> {
        // Create warehouse role
        debug!("Creating warehouse role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role_payload = NewRole {
            name: StoresRole::StoreManager,
            data: store_id,
        };
        let role = Role {
            id: new_role_id,
            user_id,
            role: role_payload.clone(),
        };

        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::WarehousesRoleSetStart(new_role_id));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to warehouses

        let client = self.http_client.clone();
        let warehouses_url = self.config.service_url(StqService::Warehouses);

        let res = serde_json::to_string(&role)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<Role>(
                        Method::Post,
                        format!("{}/{}", warehouses_url, StqModel::Role.to_url()),
                        Some(body),
                        Some(headers),
                    )
                    .map_err(|e| {
                        format_err!("Creating role in warehouses microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::WarehousesRoleSetComplete(new_role_id));
            })
            .then(|res| match res {
                Ok(warehouse_role) => Ok((self, warehouse_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_orders_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, Role> {
        // Create orders role
        debug!("Creating orders role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role_payload = NewRole {
            name: StoresRole::StoreManager,
            data: store_id,
        };
        let role = Role {
            id: new_role_id,
            user_id,
            role: role_payload.clone(),
        };

        log.lock().unwrap().push(CreateStoreOperationStage::OrdersRoleSetStart(new_role_id));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to orders

        let client = self.http_client.clone();
        let orders_url = self.config.service_url(StqService::Orders);

        let res = serde_json::to_string(&role)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<Role>(
                        Method::Post,
                        format!("{}/{}", orders_url, StqModel::Role.to_url()),
                        Some(body),
                        Some(headers),
                    )
                    .map_err(|e| {
                        format_err!("Creating role in orders microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::OrdersRoleSetComplete(new_role_id));
            })
            .then(|res| match res {
                Ok(orders_role) => Ok((self, orders_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_billing_role(self, user_id: UserId, store_id: StoreId) -> ServiceFuture<Self, BillingRole> {
        // Create billing role
        debug!("Creating billing role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role = BillingRole {
            id: new_role_id,
            user_id,
            name: StoresRole::StoreManager,
            data: Some(store_id),
        };

        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::BillingRoleSetStart(new_role_id));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to billing

        let client = self.http_client.clone();
        let billing_url = self.config.service_url(StqService::Billing);

        let res = serde_json::to_string(&role)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<BillingRole>(
                        Method::Post,
                        format!("{}/{}", billing_url, StqModel::Role.to_url()),
                        Some(body),
                        Some(headers),
                    )
                    .map_err(|e| {
                        format_err!("Creating role in billing microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::BillingRoleSetComplete(new_role_id));
            })
            .then(|res| match res {
                Ok(billing_role) => Ok((self, billing_role)),
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

        let client = self.http_client.clone();
        let billing_url = self.config.service_url(StqService::Billing);

        let res = serde_json::to_string(&payload)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                let mut headers = Headers::new();
                headers.set(Authorization("1".to_string())); // only super admin can add role to warehouses
                client
                    .request::<Merchant>(Method::Post, format!("{}/merchants/store", billing_url), Some(body), Some(headers))
                    .map_err(|e| {
                        format_err!("Creating merchant in billing microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::BillingCreateMerchantComplete(store_id));
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
                    s.create_warehouse_role(user_id, store_id).map(|(s, _)| (s, store))
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
                .and_then(|(s, store)| s.create_merchant(store.id).map(|(s, _)| (s, store))),
        )
    }

    // Contains reversal of Store creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();

        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log {
            match e {
                CreateStoreOperationStage::StoreCreationStart(user_id) => {
                    debug!("Reverting store, user_id: {}", user_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        if let Some(ref user_id) = s.user_id {
                            headers.set(Authorization(user_id.to_string()));
                        };

                        s.http_client
                            .request::<Option<Store>>(
                                Method::Delete,
                                format!(
                                    "{}/{}/by_user_id/{}",
                                    s.config.service_url(StqService::Stores),
                                    StqModel::Store.to_url(),
                                    user_id,
                                ),
                                None,
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Store service create_revert StoreCreationStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateStoreOperationStage::WarehousesRoleSetStart(role_id) => {
                    debug!("Reverting warehouses role, user_id: {}", role_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can delete role from warehouses

                        s.http_client
                            .request::<Role>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Warehouses), "roles/by-id", role_id,),
                                None,
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Store service create_revert WarehouseRoleSetStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateStoreOperationStage::OrdersRoleSetStart(role_id) => {
                    debug!("Reverting orders role, user_id: {}", role_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can delete role from orders

                        s.http_client
                            .request::<Role>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Orders), "roles/by-id", role_id),
                                None,
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Store service create_revert OrdersRoleSetStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateStoreOperationStage::BillingRoleSetStart(role_id) => {
                    debug!("Reverting billing role, user_id: {}", role_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can delete role from billing

                        s.http_client
                            .request::<Role>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Billing), "roles/by-id", role_id,),
                                None,
                                Some(headers),
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Store service create_revert BillingRoleSetStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateStoreOperationStage::BillingCreateMerchantStart(store_id) => {
                    debug!("Reverting merchant, store_id: {}", store_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        s.http_client
                            .request::<MerchantId>(
                                Method::Delete,
                                format!("{}/merchants/store/{}", s.config.service_url(StqService::Billing), store_id.0,),
                                None,
                                None,
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Account service create_revert BillingCreateMerchantStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                _ => {}
            }
        }

        fut
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
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use failure::{Context, Error as FailureError, Fail};
use futures;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;
use hyper::StatusCode;

use serde_json;
use uuid::Uuid;
use validator::{ValidationError, ValidationErrors};

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::client::Error as HttpError;
use stq_http::errors::ErrorMessage;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;

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
        let body = serde_json::to_string(input).unwrap();
        let log = self.log.clone();
        let user_id = input.user_id;
        log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationStart(user_id));

        let mut headers = Headers::new();
        if let Some(ref user_id) = self.user_id {
            headers.set(Authorization(user_id.to_string()));
        };

        let res = self.http_client
            .request::<Store>(
                Method::Post,
                format!("{}/{}", self.config.service_url(StqService::Stores), StqModel::Store.to_url()),
                Some(body),
                Some(headers),
            )
            .inspect(move |_| {
                log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationComplete(user_id));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((
                    self,
                    format_err!("Creating store in stores microservice failed.")
                        .context(Error::HttpClient(e))
                        .into(),
                )),
            });

        Box::new(res)
    }

    fn create_warehouse_role(self, user_id: i32, store_id: i32) -> ServiceFuture<Self, Role> {
        // Create warehouse role
        debug!("Creating warehouse role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = Uuid::new_v4();
        let role_payload = NewRole {
            name: "StoreManager".to_string(),
            data: store_id,
        };
        let role = Role {
            id: new_role_id.clone(),
            user_id: user_id,
            role: role_payload.clone(),
        };

        let body = serde_json::to_string(&role).unwrap_or_default();
        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::WarehousesRoleSetStart(new_role_id.clone()));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to warehouses

        let res = self.http_client
            .request::<Role>(
                Method::Post,
                format!("{}/{}", self.config.service_url(StqService::Warehouses), "roles"),
                Some(body),
                Some(headers),
            )
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::WarehousesRoleSetComplete(new_role_id.clone()));
            })
            .then(|res| match res {
                Ok(role) => Ok((self, role)),
                Err(e) => Err((
                    self,
                    format_err!("Creating role in warehouses microservice failed.")
                        .context(Error::HttpClient(e))
                        .into(),
                )),
            });

        Box::new(res)
    }

    fn create_orders_role(self, user_id: i32, store_id: i32) -> ServiceFuture<Self, Role> {
        // Create orders role
        debug!("Creating orders role, user id: {}, store id: {}", user_id, store_id);
        let log = self.log.clone();

        let new_role_id = Uuid::new_v4();
        let role_payload = NewRole {
            name: "StoreManager".to_string(),
            data: store_id,
        };
        let role = Role {
            id: new_role_id.clone(),
            user_id: user_id,
            role: role_payload.clone(),
        };

        let body = serde_json::to_string(&role).unwrap_or_default();
        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::OrdersRoleSetStart(new_role_id.clone()));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to orders

        let res = self.http_client
            .request::<Role>(
                Method::Post,
                format!("{}/{}", self.config.service_url(StqService::Orders), "roles"),
                Some(body),
                Some(headers),
            )
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::OrdersRoleSetComplete(new_role_id.clone()));
            })
            .then(|res| match res {
                Ok(role) => Ok((self, role)),
                Err(e) => Err((
                    self,
                    format_err!("Creating role in orders microservice failed.")
                        .context(Error::HttpClient(e))
                        .into(),
                )),
            });

        Box::new(res)
    }

    // Contains happy path for Store creation
    fn create_happy(self, input: NewStore) -> ServiceFuture<Self, Store> {
        Box::new(self.create_store(&input).and_then(|(s, store)| {
            let user_id = store.user_id;
            let store_id = store.id;
            s.create_warehouse_role(user_id, store_id)
                .and_then(move |(s, _)| s.create_orders_role(user_id, store_id))
                .map(|(s, _)| (s, store))
        }))
    }

    // Contains reversal of Store creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();

        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log.into_iter() {
            match e {
                CreateStoreOperationStage::OrdersRoleSetStart(role_id) => {
                    debug!("Reverting orders role, user_id: {}", role_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can delete role from orders

                        s.http_client
                            .request::<Role>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    s.config.service_url(StqService::Orders),
                                    "roles/by-id",
                                    role_id.clone(),
                                ),
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

                CreateStoreOperationStage::WarehousesRoleSetStart(role_id) => {
                    debug!("Reverting warehouses role, user_id: {}", role_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can delete role from warehouses

                        s.http_client
                            .request::<Role>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    s.config.service_url(StqService::Warehouses),
                                    "roles/by-id",
                                    role_id.clone(),
                                ),
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

                CreateStoreOperationStage::StoreCreationStart(user_id) => {
                    debug!("Reverting store, user_id: {}", user_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
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

                _ => {}
            }
        }

        fut
    }
}

impl StoreService for StoreServiceImpl {
    fn create(self, input: NewStore) -> ServiceFuture<Box<StoreService>, Option<Store>> {
        Box::new(
            self.create_happy(input.clone())
                .map(|(s, store)| (Box::new(s) as Box<StoreService>, Some(store)))
                .or_else(move |(s, e)| {
                    s.create_revert().then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        futures::future::err((Box::new(s) as Box<StoreService>, e.into()))
                    })
                })
                .map_err(|(s, e): (Box<StoreService>, FailureError)| {
                    {
                        let real_err = e.causes()
                            .filter_map(|cause| {
                                if let Some(ctx) = cause.downcast_ref::<Context<Error>>() {
                                    Some(ctx.get_context())
                                } else {
                                    cause.downcast_ref::<Error>()
                                }
                            })
                            .nth(0);
                        if let Some(Error::HttpClient(HttpError::Api(
                            _,
                            Some(ErrorMessage {
                                payload,
                                code,
                                description,
                            }),
                        ))) = real_err
                        {
                            match code {
                                x if x == &StatusCode::Forbidden.as_u16() => {
                                    return (s, format_err!("{}", description).context(Error::Forbidden).into())
                                }
                                x if x == &StatusCode::NotFound.as_u16() => {
                                    return (s, format_err!("{}", description).context(Error::NotFound).into())
                                }
                                x if x == &StatusCode::BadRequest.as_u16() => {
                                    if let Some(payload) = payload {
                                        // Wierd construction of ValidationErrors due to the fact ValidationErrors.add
                                        // only accepts str with static lifetime
                                        let valid_err_res =
                                            serde_json::from_value::<HashMap<String, Vec<ValidationError>>>(payload.clone());
                                        match valid_err_res {
                                            Ok(valid_err_map) => {
                                                let mut valid_errors = ValidationErrors::new();
                                                if let Some(map_val) = valid_err_map.get("name") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("name", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("short_description") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("short_description", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("long_description") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("long_description", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("slug") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("slug", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("phone") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("phone", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("email") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("email", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("default_language") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("default_language", map_val[0].clone())
                                                    }
                                                }
                                                if let Some(map_val) = valid_err_map.get("store") {
                                                    if !map_val.is_empty() {
                                                        valid_errors.add("store", map_val[0].clone())
                                                    }
                                                }

                                                return (s, Error::Validate(valid_errors).into());
                                            }
                                            Err(e) => {
                                                return (s, e.context("Cannot parse validation errors").context(Error::Unknown).into());
                                            }
                                        }
                                    } else {
                                        return (s, format_err!("{}", description).context(Error::Unknown).into());
                                    }
                                }
                                _ => return (s, format_err!("{}", description).context(Error::Unknown).into()),
                            }
                        }
                    }
                    (s, e)
                }),
        )
    }
}

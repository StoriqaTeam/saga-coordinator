use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use failure::{Context, Error as FailureError, Fail};
use futures;
use futures::prelude::*;
use hyper::Method;
use serde_json;
use validator::{ValidationError, ValidationErrors};

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::client::Error as HttpError;
use stq_http::errors::ErrorMessage;
use stq_routes::model::Model as StqModel;
use stq_routes::role::UserRole as StqUserRole;
use stq_routes::service::Service as StqService;

use config;
use errors::Error;
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
}

impl StoreServiceImpl {
    pub fn new(http_client: Arc<HttpClientHandle>, config: config::Config) -> Self {
        let log = Arc::new(Mutex::new(CreateStoreOperationLog::new()));
        Self { http_client, config, log }
    }

    fn create_store(self, input: NewStore) -> ServiceFuture<Self, Store> {
        // Create Store
        let body = serde_json::to_string(&input).unwrap();
        let log = self.log.clone();
        let user_id = input.user_id;
        log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationStart(user_id));

        let res = self.http_client
            .request::<Store>(
                Method::Post,
                format!("{}/{}", self.config.service_url(StqService::Stores), StqModel::Store.to_url()),
                Some(body),
                None,
            )
            .inspect(move |_| {
                log.lock().unwrap().push(CreateStoreOperationStage::StoreCreationComplete(user_id));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((
                    self,
                    format_err!("Creating user in users microservice failed.")
                        .context(Error::HttpClient(e))
                        .into(),
                )),
            });

        Box::new(res)
    }

    fn create_warehouse_role(self, user_id: i32, store_id: i32) -> ServiceFuture<Self, StqUserRole> {
        // Create Store
        let log = self.log.clone();
        let body = json!({"role_id": "store_manager", "role_data": store_id}).to_string();
        log.lock()
            .unwrap()
            .push(CreateStoreOperationStage::WarehouseRoleSetStart(user_id.clone()));

        let res = self.http_client
            .request::<StqUserRole>(
                Method::Post,
                format!(
                    "{}/{}/{}",
                    self.config.service_url(StqService::Warehouses),
                    "roles/by-user-id/",
                    user_id.clone()
                ),
                Some(body),
                None,
            )
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateStoreOperationStage::WarehouseRoleSetComplete(user_id.clone()));
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

    // Contains happy path for Store creation
    fn create_happy(self, input: NewStore) -> ServiceFuture<Self, Store> {
        Box::new(
            self.create_store(input)
                .and_then({ |(s, store)| s.create_warehouse_role(store.user_id, store.id).map(|(s, _)| (s, store)) }),
        )
    }

    // Contains reversal of Store creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();
        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log.into_iter() {
            match e {
                CreateStoreOperationStage::WarehouseRoleSetStart(user_id) => {
                    println!("Reverting users role, user_id: {}", user_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
                        s.http_client
                            .request::<StqUserRole>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    s.config.service_url(StqService::Warehouses),
                                    "roles/default",
                                    user_id.clone(),
                                ),
                                None,
                                None,
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
                    println!("Reverting user, user_id: {}", user_id);
                    fut = Box::new(fut.and_then(move |(s, _)| {
                        s.http_client
                            .request::<StqUserRole>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    s.config.service_url(StqService::Stores),
                                    "user_by_user_id",
                                    user_id.clone(),
                                ),
                                None,
                                None,
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
                .map(|(s, user)| (Box::new(s) as Box<StoreService>, Some(user)))
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
                        if let Some(Error::HttpClient(HttpError::Api(_, Some(ErrorMessage { payload, .. })))) = real_err {
                            if let Some(payload) = payload {
                                // Wierd construction of ValidationErrors due to the fact ValidationErrors.add
                                // only accepts str with static lifetime
                                let valid_err_res = serde_json::from_value::<HashMap<String, Vec<ValidationError>>>(payload.clone());
                                match valid_err_res {
                                    Ok(valid_err_map) => {
                                        let mut valid_errors = ValidationErrors::new();

                                        if let Some(map_val) = valid_err_map.get("email") {
                                            if !map_val.is_empty() {
                                                valid_errors.add("email", map_val[0].clone())
                                            }
                                        }

                                        if let Some(map_val) = valid_err_map.get("password") {
                                            if !map_val.is_empty() {
                                                valid_errors.add("password", map_val[0].clone())
                                            }
                                        }

                                        return (s, Error::Validate(valid_errors).into());
                                    }
                                    Err(e) => {
                                        return (s, e.context("Cannot parse validation errors").into());
                                    }
                                }
                            } else {
                                return (s, format_err!("Http error does not contain payload. ").into());
                            }
                        }
                    }
                    (s, e)
                }),
        )
    }
}

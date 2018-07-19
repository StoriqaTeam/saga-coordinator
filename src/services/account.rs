use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use failure::{Context, Error as FailureError, Fail};
use futures;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;
use serde_json;
use validator::{ValidationError, ValidationErrors};

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::client::Error as HttpError;
use stq_http::errors::ErrorMessage;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_types::{MerchantId, SagaId, UserId};

use config;
use errors::Error;
use models::create_profile::UserRole as StqUserRole;
use models::*;
use services::types::ServiceFuture;

pub trait AccountService {
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Box<AccountService>, Option<User>>;
}

/// Account service, responsible for Creating user
pub struct AccountServiceImpl {
    pub http_client: Arc<HttpClientHandle>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateProfileOperationLog>>,
}

impl AccountServiceImpl {
    pub fn new(http_client: Arc<HttpClientHandle>, config: config::Config) -> Self {
        let log = Arc::new(Mutex::new(CreateProfileOperationLog::new()));
        Self { http_client, config, log }
    }

    fn create_user(self, input: SagaCreateProfile, saga_id_arg: SagaId) -> ServiceFuture<Self, User> {
        debug!("Creating user, input: {}, saga id: {}", input, saga_id_arg);
        // Create account
        let new_ident = NewIdentity {
            provider: input.identity.provider,
            email: input.identity.email,
            password: input.identity.password,
            saga_id: saga_id_arg.clone(),
        };
        let new_user = input.user.clone().map(|input_user| NewUser {
            email: input_user.email.clone(),
            phone: input_user.phone.clone(),
            first_name: input_user.first_name.clone(),
            last_name: input_user.last_name.clone(),
            middle_name: input_user.middle_name.clone(),
            gender: input_user.gender.clone(),
            birthdate: input_user.birthdate,
            last_login_at: input_user.last_login_at,
            saga_id: saga_id_arg.clone(),
        });
        let create_profile = SagaCreateProfile {
            user: new_user,
            identity: new_ident,
        };

        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::AccountCreationStart(saga_id_arg.clone()));

        let client = self.http_client.clone();
        let users_url = self.config.service_url(StqService::Users);

        let res = serde_json::to_string(&create_profile)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<User>(Method::Post, format!("{}/{}", users_url, StqModel::User.to_url()), Some(body), None)
                    .map_err(|e| {
                        format_err!("Creating user in users microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::AccountCreationComplete(saga_id_arg.clone()));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_user_role(self, user_id: UserId) -> ServiceFuture<Self, StqUserRole> {
        debug!("Creating user role for user_id: {} in users microservice", user_id);
        // Create user role
        let log = self.log.clone();
        log.lock().unwrap().push(CreateProfileOperationStage::UsersRoleSetStart(user_id));

        let res = self.http_client
            .request::<StqUserRole>(
                Method::Post,
                format!("{}/{}/{}", self.config.service_url(StqService::Users), "roles/default", user_id),
                None,
                None,
            )
            .inspect(move |_| {
                log.lock().unwrap().push(CreateProfileOperationStage::UsersRoleSetComplete(user_id));
            })
            .then(|res| match res {
                Ok(role) => Ok((self, role)),
                Err(e) => Err((
                    self,
                    format_err!("Creating role in users microservice failed.")
                        .context(Error::HttpClient(e))
                        .into(),
                )),
            });

        Box::new(res)
    }

    fn create_store_role(self, user_id: UserId) -> ServiceFuture<Self, StqUserRole> {
        debug!("Creating user role for user_id: {} in stores microservice", user_id);
        // Create store role
        let log = self.log.clone();
        log.lock().unwrap().push(CreateProfileOperationStage::StoreRoleSetStart(user_id));

        let res = self.http_client
            .request::<StqUserRole>(
                Method::Post,
                format!("{}/{}/{}", self.config.service_url(StqService::Stores), "roles/default", user_id),
                None,
                None,
            )
            .inspect(move |_| {
                log.lock().unwrap().push(CreateProfileOperationStage::StoreRoleSetComplete(user_id));
            })
            .then(|res| match res {
                Ok(role) => Ok((self, role)),
                Err(e) => Err((
                    self,
                    format_err!("Creating role in stores microservice failed.")
                        .context(Error::HttpClient(e))
                        .into(),
                )),
            });

        Box::new(res)
    }

    fn create_merchant(self, user_id: UserId) -> ServiceFuture<Self, Merchant> {
        debug!("Creating merchant for user_id: {} in billing microservice", user_id);
        let payload = CreateUserMerchantPayload { id: user_id };

        // Create user role
        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::BillingCreateMerchantStart(user_id));

        let client = self.http_client.clone();
        let billing_url = self.config.service_url(StqService::Billing);

        let res = serde_json::to_string(&payload)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                let mut headers = Headers::new();
                headers.set(Authorization("1".to_string())); // only super admin can add role to warehouses

                client
                    .request::<Merchant>(Method::Post, format!("{}/merchants/user", billing_url), Some(body), Some(headers))
                    .map_err(|e| {
                        format_err!("Creating merchant in billing microservice failed.")
                            .context(Error::HttpClient(e))
                            .into()
                    })
            })
            .inspect(move |_| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::BillingCreateMerchantComplete(user_id));
            })
            .then(|res| match res {
                Ok(user) => Ok((self, user)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    // Contains happy path for account creation
    fn create_happy(self, input: SagaCreateProfile) -> ServiceFuture<Self, User> {
        let saga_id = SagaId::new();

        Box::new(
            self.create_user(input, saga_id)
                .and_then(|(s, user)| s.create_user_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_store_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_merchant(user.id).map(|(s, _)| (s, user))),
        )
    }

    // Contains reversal of account creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();
        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log {
            match e {
                CreateProfileOperationStage::StoreRoleSetStart(user_id) => {
                    debug!("Reverting users role, user_id: {}", user_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        s.http_client
                            .request::<StqUserRole>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Stores), "roles/default", user_id,),
                                None,
                                None,
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Account service create_revert StoreRoleSetStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateProfileOperationStage::AccountCreationStart(saga_id) => {
                    debug!("Reverting user, saga_id: {}", saga_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        s.http_client
                            .request::<StqUserRole>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    s.config.service_url(StqService::Users),
                                    "user_by_saga_id",
                                    saga_id.clone(),
                                ),
                                None,
                                None,
                            )
                            .then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    format_err!("Account service create_revert AccountCreationStart error occured.")
                                        .context(Error::HttpClient(e))
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateProfileOperationStage::BillingCreateMerchantStart(user_id) => {
                    debug!("Reverting merchant, user_id: {}", user_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        s.http_client
                            .request::<MerchantId>(
                                Method::Delete,
                                format!("{}/merchants/user/{}", s.config.service_url(StqService::Billing), user_id.0,),
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

impl AccountService for AccountServiceImpl {
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Box<AccountService>, Option<User>> {
        Box::new(
            self.create_happy(input.clone())
                .map(|(s, user)| (Box::new(s) as Box<AccountService>, Some(user)))
                .or_else(move |(s, e)| {
                    s.create_revert().then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        futures::future::err((Box::new(s) as Box<AccountService>, e))
                    })
                })
                .map_err(|(s, e): (Box<AccountService>, FailureError)| {
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
                                return (s, format_err!("Http error does not contain payload. "));
                            }
                        }
                    }
                    (s, e)
                }),
        )
    }
}

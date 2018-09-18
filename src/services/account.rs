use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures;
use futures::future;
use futures::prelude::*;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;
use serde_json;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::request_util::Currency as CurrencyHeader;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_static_resources::*;
use stq_types::{MerchantId, RoleEntryId, SagaId, StoresRole, UserId};

use super::parse_validation_errors;
use config;
use errors::Error;
use models::create_profile::UserRole as StqUserRole;
use models::*;
use services::types::ServiceFuture;

pub trait AccountService {
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Box<AccountService>, User>;
    fn request_password_reset(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_password_reset_apply(self, input: PasswordResetApply) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_email_verification(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_email_verification_apply(self, input: EmailVerifyApply) -> ServiceFuture<Box<AccountService>, ()>;
}

/// Account service, responsible for Creating user
pub struct AccountServiceImpl {
    pub http_client: HttpClientHandle,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateProfileOperationLog>>,
}

impl AccountServiceImpl {
    pub fn new(http_client: HttpClientHandle, config: config::Config) -> Self {
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
            saga_id: saga_id_arg,
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
            saga_id: saga_id_arg,
        });
        let create_profile = SagaCreateProfile {
            user: new_user,
            identity: new_ident,
        };

        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::AccountCreationStart(saga_id_arg));

        let client = self.http_client.clone();
        let users_url = self.config.service_url(StqService::Users);
        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can create user

        let res = serde_json::to_string(&create_profile)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<User>(
                        Method::Post,
                        format!("{}/{}", users_url, StqModel::User.to_url()),
                        Some(body),
                        Some(headers),
                    ).map_err(|e| {
                        e.context("Creating user in users microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::AccountCreationComplete(saga_id_arg));
                Ok(res)
            }).then(|res| match res {
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

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to users

        let res = self
            .http_client
            .request::<StqUserRole>(
                Method::Post,
                format!("{}/{}/{}", self.config.service_url(StqService::Users), "roles/default", user_id),
                None,
                Some(headers),
            ).and_then(move |res| {
                log.lock().unwrap().push(CreateProfileOperationStage::UsersRoleSetComplete(user_id));
                Ok(res)
            }).then(|res| match res {
                Ok(role) => Ok((self, role)),
                Err(e) => Err((
                    self,
                    e.context("Creating role in users microservice failed.")
                        .context(Error::HttpClient)
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

        let mut headers = Headers::new();
        headers.set(CurrencyHeader("STQ".to_string())); // stores accept requests only with Currency header
        headers.set(Authorization("1".to_string())); // only super admin can add role to stores
        let res = self
            .http_client
            .request::<StqUserRole>(
                Method::Post,
                format!("{}/{}/{}", self.config.service_url(StqService::Stores), "roles/default", user_id),
                None,
                Some(headers),
            ).and_then(move |res| {
                log.lock().unwrap().push(CreateProfileOperationStage::StoreRoleSetComplete(user_id));
                Ok(res)
            }).then(|res| match res {
                Ok(role) => Ok((self, role)),
                Err(e) => Err((
                    self,
                    e.context("Creating role in stores microservice failed.")
                        .context(Error::HttpClient)
                        .into(),
                )),
            });

        Box::new(res)
    }

    fn create_billing_role(self, user_id: UserId) -> ServiceFuture<Self, BillingRole> {
        // Create billing role
        debug!("Creating billing role, user id: {}", user_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role = BillingRole {
            id: new_role_id,
            user_id,
            name: StoresRole::User,
            data: None,
        };

        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::BillingRoleSetStart(new_role_id));

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
                    ).map_err(|e| {
                        e.context("Creating role in billing microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::BillingRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(billing_role) => Ok((self, billing_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_delivery_role(self, user_id: UserId) -> ServiceFuture<Self, DeliveryRole> {
        // Create delivery role
        debug!("Creating delivery role, user id: {}", user_id);
        let log = self.log.clone();

        let new_role_id = RoleEntryId::new();
        let role = BillingRole {
            id: new_role_id,
            user_id,
            name: StoresRole::User,
            data: None,
        };

        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::DeliveryRoleSetStart(new_role_id));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to delivery

        let client = self.http_client.clone();
        let delivery_url = self.config.service_url(StqService::Delivery);

        let res = serde_json::to_string(&role)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<DeliveryRole>(
                        Method::Post,
                        format!("{}/{}", delivery_url, StqModel::Role.to_url()),
                        Some(body),
                        Some(headers),
                    ).map_err(|e| {
                        e.context("Creating role in delivery microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::DeliveryRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(delivery_role) => Ok((self, delivery_role)),
                Err(e) => Err((self, e)),
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
                        e.context("Creating merchant in billing microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::BillingCreateMerchantComplete(user_id));
                Ok(res)
            }).then(|res| match res {
                Ok(merchant) => Ok((self, merchant)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn notify_user(self, user: User) -> ServiceFuture<Self, ()> {
        debug!("Notifiing user in notificatins microservice");
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let verify_email_path = self.config.notification_urls.verify_email_path.clone();

        let url = format!("{}/{}/email_verify_token", users_url, StqModel::User.to_url());
        let reset = ResetRequest { email: user.email.clone() };
        let user_id = user.id;
        let res = serde_json::to_string(&reset)
            .map_err(From::from)
            .into_future()
            .and_then({
                let client = self.http_client.clone();
                move |body| {
                    let mut headers = Headers::new();
                    headers.set(Authorization(user_id.to_string()));
                    client.request::<String>(Method::Post, url, Some(body), Some(headers)).map_err(|e| {
                        e.context("Creating email verify token in users microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
                }
            }).and_then({
                let client = self.http_client.clone();
                move |token| {
                    let user = EmailUser {
                        email: user.email.clone(),
                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                    };
                    let email = EmailVerificationForUser {
                        user,
                        verify_email_path,
                        token,
                    };
                    let url = format!("{}/{}/email-verification", notification_url, StqModel::User.to_url(),);
                    serde_json::to_string(&email)
                        .map_err(From::from)
                        .into_future()
                        .and_then(move |body| {
                            let mut headers = Headers::new();
                            headers.set(Authorization("1".to_string())); //only superuser can send notifications
                            client.request::<()>(Method::Post, url, Some(body), Some(headers)).map_err(|e| {
                                e.context("Sending email to notifications microservice failed.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                        })
                }
            }).then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    // Contains happy path for account creation
    fn create_happy(self, input: SagaCreateProfile) -> ServiceFuture<Self, User> {
        let saga_id = SagaId::new();
        let provider = input.identity.provider.clone();

        Box::new(
            self.create_user(input, saga_id)
                .and_then(|(s, user)| s.create_user_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_store_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_billing_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_delivery_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_merchant(user.id).map(|(s, _)| (s, user)))
                .and_then(move |(s, user)| {
                    if provider == Provider::Email {
                        // only if provider is email it needs to ber verified
                        Box::new(s.notify_user(user.clone()).then(|res| match res {
                            Ok((s, _)) => Ok((s, user)),
                            Err((s, _)) => Ok((s, user)),
                        })) as ServiceFuture<Self, User>
                    } else {
                        Box::new(future::ok((s, user))) as ServiceFuture<Self, User>
                    }
                }),
        )
    }

    // Contains reversal of account creation
    fn create_revert(self) -> ServiceFuture<Self, ()> {
        let log = self.log.lock().unwrap().clone();
        let mut fut: ServiceFuture<Self, ()> = Box::new(futures::future::ok((self, ())));
        for e in log {
            match e {
                CreateProfileOperationStage::StoreRoleSetComplete(user_id) => {
                    debug!("Reverting users role, user_id: {}", user_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        headers.set(CurrencyHeader("STQ".to_string())); // stores accept requests only with Currency header
                        s.http_client
                            .request::<StqUserRole>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Stores), "roles/default", user_id,),
                                None,
                                Some(headers),
                            ).then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    e.context("Account service create_revert StoreRoleSetStart error occured.")
                                        .context(Error::HttpClient)
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateProfileOperationStage::AccountCreationComplete(saga_id) => {
                    debug!("Reverting user, saga_id: {}", saga_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        s.http_client
                            .request::<StqUserRole>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Users), "user_by_saga_id", saga_id,),
                                None,
                                None,
                            ).then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    e.context("Account service create_revert AccountCreationStart error occured.")
                                        .context(Error::HttpClient)
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateProfileOperationStage::BillingRoleSetComplete(role_id) => {
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
                            ).then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    e.context("Store service create_revert BillingRoleSetStart error occured.")
                                        .context(Error::HttpClient)
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateProfileOperationStage::DeliveryRoleSetComplete(role_id) => {
                    debug!("Reverting delivery role, user_id: {}", role_id);
                    fut = Box::new(fut.then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string())); // only super admin can delete role from delivery

                        s.http_client
                            .request::<Role>(
                                Method::Delete,
                                format!("{}/{}/{}", s.config.service_url(StqService::Delivery), "roles/by-id", role_id,),
                                None,
                                Some(headers),
                            ).then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    e.context("Store service create_revert DeliveryRoleSetStart error occured.")
                                        .context(Error::HttpClient)
                                        .into(),
                                )),
                            })
                    }));
                }

                CreateProfileOperationStage::BillingCreateMerchantComplete(user_id) => {
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
                            ).then(|res| match res {
                                Ok(_) => Ok((s, ())),
                                Err(e) => Err((
                                    s,
                                    e.context(format_err!(
                                        "Account service create_revert BillingCreateMerchantStart error occured."
                                    )).context(Error::HttpClient)
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
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Box<AccountService>, User> {
        Box::new(
            self.create_happy(input.clone())
                .map(|(s, user)| (Box::new(s) as Box<AccountService>, user))
                .or_else(move |(s, e)| {
                    s.create_revert().then(move |res| {
                        let s = match res {
                            Ok((s, _)) => s,
                            Err((s, _)) => s,
                        };
                        futures::future::err((Box::new(s) as Box<AccountService>, e))
                    })
                }).map_err(|(s, e): (Box<AccountService>, FailureError)| (s, parse_validation_errors(e, &["email", "password"]))),
        )
    }

    fn request_password_reset(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()> {
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let reset_password_path = self.config.notification_urls.reset_password_path.clone();
        let client = self.http_client.clone();

        let url = format!("{}/{}/by_email?email={}", users_url, StqModel::User.to_url(), input.email);
        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string()));
        let res = client
            .request::<Option<User>>(Method::Get, url, None, Some(headers))
            .map_err(|e| {
                e.context("Receiving user from users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |user| {
                if let Some(user) = user {
                    if user.is_blocked {
                        return Box::new(future::err(
                            Error::Validate(validation_errors!({"email": ["email" => "Email is blocked"]})).into(),
                        )) as Box<Future<Item = (), Error = FailureError>>;
                    }

                    let user_id = user.id;
                    let url = format!("{}/{}/password_reset_token", users_url, StqModel::User.to_url());

                    Box::new(
                        serde_json::to_string(&input)
                            .map_err(From::from)
                            .into_future()
                            .and_then({
                                let client = client.clone();
                                move |body| {
                                    let mut headers = Headers::new();
                                    headers.set(Authorization(user_id.to_string()));
                                    client.request::<String>(Method::Post, url, Some(body), Some(headers)).map_err(|e| {
                                        e.context("Creating password reset token in users microservice failed.")
                                            .context(Error::HttpClient)
                                            .into()
                                    })
                                }
                            }).and_then({
                                let client = client.clone();
                                move |token| {
                                    let user = EmailUser {
                                        email: user.email.clone(),
                                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                                    };
                                    let email = PasswordResetForUser {
                                        user,
                                        reset_password_path,
                                        token,
                                    };
                                    let url = format!("{}/{}/password-reset", notification_url, StqModel::User.to_url());
                                    serde_json::to_string(&email)
                                        .map_err(From::from)
                                        .into_future()
                                        .and_then(move |body| {
                                            let mut headers = Headers::new();
                                            headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                            client
                                                .request::<()>(Method::Post, url, Some(body), Some(headers))
                                                .map_err(|e| e.context("Sending notification failed.").context(Error::HttpClient).into())
                                        })
                                }
                            }),
                    )
                } else {
                    Box::new(future::err(
                        Error::Validate(validation_errors!({"email": ["email" => "Email does not exists"]})).into(),
                    )) as Box<Future<Item = (), Error = FailureError>>
                }
            }).then(|res| match res {
                Ok(_) => Ok((Box::new(self) as Box<AccountService>, ())),
                Err(e) => Err((Box::new(self) as Box<AccountService>, parse_validation_errors(e, &["email"]))),
            });

        Box::new(res)
    }

    fn request_password_reset_apply(self, input: PasswordResetApply) -> ServiceFuture<Box<AccountService>, ()> {
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let client = self.http_client.clone();
        let url = format!("{}/{}/password_reset_token", users_url, StqModel::User.to_url());
        Box::new(
            serde_json::to_string(&input)
                .map_err(From::from)
                .into_future()
                .and_then({
                    let client = client.clone();
                    move |body| {
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string()));
                        client.request::<String>(Method::Put, url, Some(body), Some(headers)).map_err(|e| {
                            e.context("Applying password reset token in users microservice failed.")
                                .context(Error::HttpClient)
                                .into()
                        })
                    }
                }).and_then({
                    let client = client.clone();
                    move |email| {
                        let url = format!("{}/{}/by_email?email={}", users_url, StqModel::User.to_url(), email);
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string()));
                        client.request::<Option<User>>(Method::Get, url, None, Some(headers)).map_err(|e| {
                            e.context("Receiving user from users microservice failed.")
                                .context(Error::HttpClient)
                                .into()
                        })
                    }
                }).and_then({
                    let client = client.clone();
                    move |user| {
                        if let Some(user) = user {
                            let user = EmailUser {
                                email: user.email.clone(),
                                first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                            };
                            let email = ApplyPasswordResetForUser { user };
                            let url = format!("{}/{}/apply-password-reset", notification_url, StqModel::User.to_url());
                            Box::new(
                                serde_json::to_string(&email)
                                    .map_err(From::from)
                                    .into_future()
                                    .and_then(move |body| {
                                        let mut headers = Headers::new();
                                        headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                        client
                                            .request::<()>(Method::Post, url, Some(body), Some(headers))
                                            .map_err(|e| e.context("Sending notification failed.").context(Error::HttpClient).into())
                                    }),
                            )
                        } else {
                            Box::new(future::err(
                                Error::Validate(validation_errors!({"email": ["email" => "Email does not exists"]})).into(),
                            )) as Box<Future<Item = (), Error = FailureError>>
                        }
                    }
                }).then(|res| match res {
                    Ok(_) => Ok((Box::new(self) as Box<AccountService>, ())),
                    Err(e) => Err((Box::new(self) as Box<AccountService>, e)),
                }),
        )
    }

    fn request_email_verification(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()> {
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let verify_email_path = self.config.notification_urls.verify_email_path.clone();
        let client = self.http_client.clone();

        let url = format!("{}/{}/by_email?email={}", users_url, StqModel::User.to_url(), input.email);
        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string()));
        let res = client
            .request::<Option<User>>(Method::Get, url, None, Some(headers))
            .map_err(|e| {
                e.context("Receiving user from users microservice failed.")
                    .context(Error::HttpClient)
                    .into()
            }).and_then(move |user| {
                if let Some(user) = user {
                    if user.is_blocked {
                        return Box::new(future::err(
                            Error::Validate(validation_errors!({"email": ["email" => "Email is blocked"]})).into(),
                        )) as Box<Future<Item = (), Error = FailureError>>;
                    }

                    let user_id = user.id;
                    let url = format!("{}/{}/email_verify_token", users_url, StqModel::User.to_url());

                    Box::new(
                        serde_json::to_string(&input)
                            .map_err(From::from)
                            .into_future()
                            .and_then({
                                let client = client.clone();
                                move |body| {
                                    let mut headers = Headers::new();
                                    headers.set(Authorization(user_id.to_string()));
                                    client.request::<String>(Method::Post, url, Some(body), Some(headers)).map_err(|e| {
                                        e.context("Creating email verification token in users microservice failed.")
                                            .context(Error::HttpClient)
                                            .into()
                                    })
                                }
                            }).and_then({
                                let client = client.clone();
                                move |token| {
                                    let user = EmailUser {
                                        email: user.email.clone(),
                                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                                    };
                                    let email = EmailVerificationForUser {
                                        user,
                                        verify_email_path,
                                        token,
                                    };
                                    let url = format!("{}/{}/email-verification", notification_url, StqModel::User.to_url());
                                    serde_json::to_string(&email)
                                        .map_err(From::from)
                                        .into_future()
                                        .and_then(move |body| {
                                            let mut headers = Headers::new();
                                            headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                            client
                                                .request::<()>(Method::Post, url, Some(body), Some(headers))
                                                .map_err(|e| e.context("Sending notification failed.").context(Error::HttpClient).into())
                                        })
                                }
                            }),
                    )
                } else {
                    Box::new(future::err(
                        Error::Validate(validation_errors!({"email": ["email" => "Email does not exists"]})).into(),
                    )) as Box<Future<Item = (), Error = FailureError>>
                }
            }).then(|res| match res {
                Ok(_) => Ok((Box::new(self) as Box<AccountService>, ())),
                Err(e) => Err((Box::new(self) as Box<AccountService>, e)),
            });

        Box::new(res)
    }

    fn request_email_verification_apply(self, input: EmailVerifyApply) -> ServiceFuture<Box<AccountService>, ()> {
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let client = self.http_client.clone();

        let url = format!("{}/{}/email_verify_token?token={}", users_url, StqModel::User.to_url(), input.token);
        Box::new(
            serde_json::to_string(&input)
                .map_err(From::from)
                .into_future()
                .and_then({
                    let client = client.clone();
                    move |body| {
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string()));
                        client.request::<String>(Method::Put, url, Some(body), Some(headers)).map_err(|e| {
                            e.context("Applying email verification token in users microservice failed.")
                                .context(Error::HttpClient)
                                .into()
                        })
                    }
                }).and_then({
                    let client = client.clone();
                    move |email| {
                        let url = format!("{}/{}/by_email?email={}", users_url, StqModel::User.to_url(), email);
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string()));
                        client.request::<Option<User>>(Method::Get, url, None, Some(headers)).map_err(|e| {
                            e.context("Receiving user from users microservice failed.")
                                .context(Error::HttpClient)
                                .into()
                        })
                    }
                }).and_then({
                    let client = client.clone();
                    move |user| {
                        if let Some(user) = user {
                            let user = EmailUser {
                                email: user.email.clone(),
                                first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                            };
                            let email = ApplyEmailVerificationForUser { user };
                            let url = format!("{}/{}/apply-email-verification", notification_url, StqModel::User.to_url());
                            Box::new(
                                serde_json::to_string(&email)
                                    .map_err(From::from)
                                    .into_future()
                                    .and_then(move |body| {
                                        let mut headers = Headers::new();
                                        headers.set(Authorization("1".to_string())); //only superuser can send notifications
                                        client
                                            .request::<()>(Method::Post, url, Some(body), Some(headers))
                                            .map_err(|e| e.context("Sending notification failed.").into())
                                    }),
                            )
                        } else {
                            Box::new(future::err(
                                Error::Validate(validation_errors!({"email": ["email" => "Email does not exists"]})).into(),
                            )) as Box<Future<Item = (), Error = FailureError>>
                        }
                    }
                }).then(|res| match res {
                    Ok(_) => Ok((Box::new(self) as Box<AccountService>, ())),
                    Err(e) => Err((Box::new(self) as Box<AccountService>, e)),
                }),
        )
    }
}

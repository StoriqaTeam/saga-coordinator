use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use failure::Fail;
use futures;
use futures::future;
use futures::prelude::*;
use futures::stream::iter_ok;
use hyper::header::Authorization;
use hyper::Headers;
use hyper::Method;
use serde_json;

use stq_http::client::ClientHandle as HttpClientHandle;
use stq_http::request_util::Currency as CurrencyHeader;
use stq_routes::model::Model as StqModel;
use stq_routes::service::Service as StqService;
use stq_static_resources::*;
use stq_types::{BillingRole, DeliveryRole, MerchantId, RoleId, SagaId, StoresRole, UserId, UsersRole};

use super::parse_validation_errors;
use config;
use errors::Error;
use models::*;
use services::types::ServiceFuture;

pub trait AccountService {
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Box<AccountService>, User>;
    fn request_password_reset(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_password_reset_apply(self, input: PasswordResetApply) -> ServiceFuture<Box<AccountService>, String>;
    fn request_email_verification(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_email_verification_apply(self, input: EmailVerifyApply) -> ServiceFuture<Box<AccountService>, String>;
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
            device: input.device.clone(),
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

    fn create_user_role(self, user_id: UserId) -> ServiceFuture<Self, NewRole<UsersRole>> {
        debug!("Creating user role for user_id: {} in users microservice", user_id);
        // Create user role
        let log = self.log.clone();

        let new_role_id = RoleId::new();
        let role = NewRole::<UsersRole>::new(new_role_id, user_id, UsersRole::User, None);

        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::UsersRoleSetStart(new_role_id));

        let mut headers = Headers::new();
        headers.set(Authorization("1".to_string())); // only super admin can add role to users

        let client = self.http_client.clone();
        let users_url = self.config.service_url(StqService::Users);

        let res = serde_json::to_string(&role)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<NewRole<UsersRole>>(
                        Method::Post,
                        format!("{}/{}", users_url, StqModel::Role.to_url()),
                        Some(body),
                        Some(headers),
                    ).map_err(|e| {
                        e.context("Creating role in users microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::UsersRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(users_role) => Ok((self, users_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_store_role(self, user_id: UserId) -> ServiceFuture<Self, NewRole<StoresRole>> {
        debug!("Creating user role for user_id: {} in stores microservice", user_id);
        // Create store role
        let log = self.log.clone();

        let new_role_id = RoleId::new();
        let role = NewRole::<StoresRole>::new(new_role_id, user_id, StoresRole::User, None);

        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::StoreRoleSetStart(new_role_id));

        let mut headers = Headers::new();
        headers.set(CurrencyHeader("STQ".to_string())); // stores accept requests only with Currency header
        headers.set(Authorization("1".to_string())); // only super admin can add role to stores

        let client = self.http_client.clone();
        let stores_url = self.config.service_url(StqService::Stores);

        let res = serde_json::to_string(&role)
            .into_future()
            .map_err(From::from)
            .and_then(move |body| {
                client
                    .request::<NewRole<StoresRole>>(
                        Method::Post,
                        format!("{}/{}", stores_url, StqModel::Role.to_url()),
                        Some(body),
                        Some(headers),
                    ).map_err(|e| {
                        e.context("Creating role in stores microservice failed.")
                            .context(Error::HttpClient)
                            .into()
                    })
            }).and_then(move |res| {
                log.lock()
                    .unwrap()
                    .push(CreateProfileOperationStage::StoreRoleSetComplete(new_role_id));
                Ok(res)
            }).then(|res| match res {
                Ok(stores_role) => Ok((self, stores_role)),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    fn create_billing_role(self, user_id: UserId) -> ServiceFuture<Self, NewRole<BillingRole>> {
        // Create billing role
        debug!("Creating billing role, user id: {}", user_id);
        let log = self.log.clone();

        let new_role_id = RoleId::new();
        let role = NewRole::<BillingRole>::new(new_role_id, user_id, BillingRole::User, None);

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
                    .request::<NewRole<BillingRole>>(
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

    fn create_delivery_role(self, user_id: UserId) -> ServiceFuture<Self, NewRole<DeliveryRole>> {
        // Create delivery role
        debug!("Creating delivery role, user id: {}", user_id);
        let log = self.log.clone();

        let new_role_id = RoleId::new();
        let role = NewRole::<DeliveryRole>::new(new_role_id, user_id, DeliveryRole::User, None);

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
                    .request::<NewRole<DeliveryRole>>(
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

    fn notify_user(self, user: User, device: Option<Device>) -> ServiceFuture<Self, ()> {
        debug!("Notifiing user in notificatins microservice");
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let config::DevicesUrls { web, ios, android } = self.config.notification_urls.verify_email.clone();
        let verify_email_path = device
            .map(|device| match device {
                Device::WEB => web.clone(),
                Device::IOS => ios,
                Device::Android => android,
            }).unwrap_or_else(|| web);

        let url = format!("{}/{}/email_verify_token", users_url, StqModel::User.to_url());
        let reset = ResetRequest {
            email: user.email.clone(),
            device: device,
        };
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
        let device = input.device.clone();

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
                        Box::new(s.notify_user(user.clone(), device).then(|res| match res {
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
    fn create_revert(self) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let log = self.log.lock().unwrap().clone();
        let http_client = self.http_client.clone();
        let billing_url = self.config.service_url(StqService::Billing);
        let users_url = self.config.service_url(StqService::Users);
        let stores_url = self.config.service_url(StqService::Stores);
        let delivery_url = self.config.service_url(StqService::Delivery);

        let fut = iter_ok::<_, ()>(log).for_each(move |e| {
            match e {
                CreateProfileOperationStage::AccountCreationComplete(saga_id) => {
                    debug!("Reverting user, saga_id: {}", saga_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin can delete user

                    Box::new(
                        http_client
                            .request::<User>(
                                Method::Delete,
                                format!("{}/user_by_saga_id/{}", users_url, saga_id),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::UsersRoleSetComplete(role_id) => {
                    debug!("Reverting users role, role_id: {}", role_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete user role

                    Box::new(
                        http_client
                            .request::<NewRole<UsersRole>>(
                                Method::Delete,
                                format!("{}/roles/by-id/{}", users_url, role_id),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::StoreRoleSetComplete(role_id) => {
                    debug!("Reverting stores users role, role_id: {}", role_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete user role

                    Box::new(
                        http_client
                            .request::<NewRole<StoresRole>>(
                                Method::Delete,
                                format!("{}/roles/by-id/{}", stores_url, role_id),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::BillingRoleSetComplete(role_id) => {
                    debug!("Reverting billing role, role_id: {}", role_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete user role

                    Box::new(
                        http_client
                            .request::<NewRole<BillingRole>>(
                                Method::Delete,
                                format!("{}/roles/by-id/{}", billing_url, role_id),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::DeliveryRoleSetComplete(role_id) => {
                    debug!("Reverting delivery role, role_id: {}", role_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete user role

                    Box::new(
                        http_client
                            .request::<NewRole<DeliveryRole>>(
                                Method::Delete,
                                format!("{}/roles/by-id/{}", delivery_url, role_id),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::BillingCreateMerchantComplete(user_id) => {
                    debug!("Reverting merchant, user_id: {}", user_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete merchant

                    Box::new(
                        http_client
                            .request::<MerchantId>(
                                Method::Delete,
                                format!("{}/merchants/user/{}", billing_url, user_id.0),
                                None,
                                Some(headers),
                            ).then(|_| Ok(())),
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
        let config::DevicesUrls { web, ios, android } = self.config.notification_urls.reset_password.clone();
        let reset_password_path = input
            .device
            .clone()
            .map(|device| match device {
                Device::WEB => web.clone(),
                Device::IOS => ios,
                Device::Android => android,
            }).unwrap_or_else(|| web);

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

    fn request_password_reset_apply(self, input: PasswordResetApply) -> ServiceFuture<Box<AccountService>, String> {
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let client = self.http_client.clone();
        let cluster_url = self.config.cluster.url.clone();
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
                        client
                            .request::<ResetApplyToken>(Method::Put, url, Some(body), Some(headers))
                            .map_err(|e| {
                                e.context("Applying password reset token in users microservice failed.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                    }
                }).and_then({
                    let client = client.clone();
                    move |reset_token| {
                        let url = format!("{}/{}/by_email?email={}", users_url, StqModel::User.to_url(), reset_token.email);
                        let mut headers = Headers::new();
                        headers.set(Authorization("1".to_string()));
                        client
                            .request::<Option<User>>(Method::Get, url, None, Some(headers))
                            .map_err(|e| {
                                e.context("Receiving user from users microservice failed.")
                                    .context(Error::HttpClient)
                                    .into()
                            }).map(|user| (user, reset_token.token))
                    }
                }).and_then({
                    let client = client.clone();
                    move |(user, token)| {
                        if let Some(user) = user {
                            let user = EmailUser {
                                email: user.email.clone(),
                                first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                                last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                            };
                            let email = ApplyPasswordResetForUser { user, cluster_url };
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
                                            .map(|_| token)
                                    }),
                            )
                        } else {
                            Box::new(future::err(
                                Error::Validate(validation_errors!({"email": ["email" => "Email does not exists"]})).into(),
                            )) as Box<Future<Item = String, Error = FailureError>>
                        }
                    }
                }).then(|res| match res {
                    Ok(token) => Ok((Box::new(self) as Box<AccountService>, token)),
                    Err(e) => Err((Box::new(self) as Box<AccountService>, e)),
                }),
        )
    }

    fn request_email_verification(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()> {
        let users_url = self.config.service_url(StqService::Users);
        let notification_url = self.config.service_url(StqService::Notifications);
        let config::DevicesUrls { web, ios, android } = self.config.notification_urls.verify_email.clone();
        let verify_email_path = input
            .device
            .clone()
            .map(|device| match device {
                Device::WEB => web.clone(),
                Device::IOS => ios,
                Device::Android => android,
            }).unwrap_or_else(|| web);

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

    fn request_email_verification_apply(self, input: EmailVerifyApply) -> ServiceFuture<Box<AccountService>, String> {
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
                        client
                            .request::<EmailVerifyApplyToken>(Method::Put, url, Some(body), Some(headers))
                            .map_err(|e| {
                                e.context("Applying email verification token in users microservice failed.")
                                    .context(Error::HttpClient)
                                    .into()
                            })
                    }
                }).and_then({
                    let client = client.clone();
                    move |email_apply_token| {
                        let EmailVerifyApplyToken { user, token } = email_apply_token;
                        let email_user = EmailUser {
                            email: user.email.clone(),
                            first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                            last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                        };
                        let email = ApplyEmailVerificationForUser { user: email_user };
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
                                }).map(|_| token),
                        )
                    }
                }).then(|res| match res {
                    Ok(token) => Ok((Box::new(self) as Box<AccountService>, token)),
                    Err(e) => Err((Box::new(self) as Box<AccountService>, e)),
                }),
        )
    }
}

use std::sync::{Arc, Mutex};

use failure::Error as FailureError;
use futures;
use futures::future;
use futures::prelude::*;
use futures::stream::iter_ok;
use hyper::header::Authorization;
use hyper::Headers;

use stq_static_resources::*;
use stq_types::{BillingRole, DeliveryRole, RoleId, SagaId, StoresRole, UserId, UsersRole};

use super::parse_validation_errors;
use config;
use errors::Error;
use microservice::*;
use models::*;
use services::types::ServiceFuture;

pub trait AccountService {
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Box<AccountService>, User>;
    fn request_password_reset(self, input: ResetRequest) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_password_reset_apply(self, input: PasswordResetApply) -> ServiceFuture<Box<AccountService>, String>;
    fn request_email_verification(self, input: VerifyRequest) -> ServiceFuture<Box<AccountService>, ()>;
    fn request_email_verification_apply(self, input: EmailVerifyApply) -> ServiceFuture<Box<AccountService>, String>;
}

/// Account service, responsible for Creating user
pub struct AccountServiceImpl {
    pub stores_microservice: Arc<StoresMicroservice>,
    pub billing_microservice: Arc<BillingMicroservice>,
    pub delivery_microservice: Arc<DeliveryMicroservice>,
    pub users_microservice: Arc<UsersMicroservice>,
    pub notifications_microservice: Arc<NotificationsMicroservice>,
    pub config: config::Config,
    pub log: Arc<Mutex<CreateProfileOperationLog>>,
}

impl AccountServiceImpl {
    pub fn new(
        config: config::Config,
        stores_microservice: Arc<StoresMicroservice>,
        billing_microservice: Arc<BillingMicroservice>,
        delivery_microservice: Arc<DeliveryMicroservice>,
        users_microservice: Arc<UsersMicroservice>,
        notifications_microservice: Arc<NotificationsMicroservice>,
    ) -> Self {
        let log = Arc::new(Mutex::new(CreateProfileOperationLog::new()));
        Self {
            config,
            log,
            stores_microservice,
            billing_microservice,
            delivery_microservice,
            users_microservice,
            notifications_microservice,
        }
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
            project: input.project.clone(),
        };

        let log = self.log.clone();
        log.lock()
            .unwrap()
            .push(CreateProfileOperationStage::AccountCreationStart(saga_id_arg));

        let res = self
            .users_microservice
            .create_user(Some(Initiator::Superadmin), create_profile)
            .and_then(move |res| {
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

        let res = self
            .users_microservice
            .create_role(Some(Initiator::Superadmin), role)
            .and_then(move |res| {
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

        let res = self
            .stores_microservice
            .create_stores_role(Some(Initiator::Superadmin), role)
            .and_then(move |res| {
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

        let res = self
            .billing_microservice
            .create_role(Some(Initiator::Superadmin), role)
            .and_then(move |res| {
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

        let res = self
            .delivery_microservice
            .create_delivery_role(Some(Initiator::Superadmin), role)
            .and_then(move |res| {
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

        let res = self
            .billing_microservice
            .create_user_merchant(Some(Initiator::Superadmin), payload)
            .and_then(move |res| {
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

    fn notify_user(self, user: User, device: Option<Device>, project: Option<Project>) -> ServiceFuture<Self, ()> {
        debug!("Notifiing user in notificatins microservice");
        let project_ = project.unwrap_or_else(|| Project::MarketPlace);
        let verify_email_path = match project_ {
            Project::MarketPlace => {
                let config::DevicesUrls { web, ios, android } = self.config.notification_urls.verify_email.marketplace.clone();
                device
                    .map(|device| match device {
                        Device::WEB => web.clone(),
                        Device::IOS => ios,
                        Device::Android => android,
                    }).unwrap_or_else(|| web)
            }
            Project::Wallet => {
                let config::DevicesUrls { web, ios, android } = self.config.notification_urls.verify_email.wallet.clone();
                device
                    .map(|device| match device {
                        Device::WEB => web.clone(),
                        Device::IOS => ios,
                        Device::Android => android,
                    }).unwrap_or_else(|| web)
            }
        };

        let verify = VerifyRequest {
            email: user.email.clone(),
            device: device,
            project: project,
        };
        let user_id = user.id;
        let notifications_microservice = self.notifications_microservice.clone();
        let res = self
            .users_microservice
            .create_email_verify_token(Some(user_id.into()), verify)
            .and_then(move |token| {
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
                notifications_microservice.email_verification(Some(Initiator::Superadmin), email, project_)
            }).then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(e) => Err((self, e)),
            });

        Box::new(res)
    }

    // Create new user in emarsys and update user with emarsys_id
    fn create_emarsys_contact(self, create_emarsys_payload: CreateEmarsysContactPayload) -> ServiceFuture<Self, ()> {
        let user_id = create_emarsys_payload.user_id;
        let notifications_microservice = self.notifications_microservice.clone();
        let users_microservice = self.users_microservice.clone();
        let res = notifications_microservice
            .emarsys_create_contact(create_emarsys_payload)
            .inspect(|created_contact| {
                info!(
                    "Successfully created new contact {} in emarsys for user {}",
                    created_contact.emarsys_id, created_contact.user_id
                );
            }).map(|created_contact| created_contact.emarsys_id)
            .and_then(move |emarsys_id| {
                users_microservice.update_user(
                    Some(Initiator::Superadmin),
                    user_id,
                    UpdateUser {
                        emarsys_id: Some(emarsys_id),
                        ..Default::default()
                    },
                )
            }).inspect(|user| {
                info!("Successfully changed emarsys emarsys_id for user {}", user.id);
            }).then(|res| match res {
                Ok(_) => Ok((self, ())),
                Err(error) => {
                    error!("Failed to create new contact in emarsys: {:?}", error);
                    Err((self, error))
                }
            });
        Box::new(res)
    }

    // Contains happy path for account creation
    fn create_happy(self, input: SagaCreateProfile) -> ServiceFuture<Self, User> {
        let saga_id = SagaId::new();
        let provider = input.identity.provider.clone();
        let device = input.device.clone();
        let project = input.project.clone();

        Box::new(
            self.create_user(input, saga_id)
                .and_then(|(s, user)| s.create_user_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_store_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_billing_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_delivery_role(user.id).map(|(s, _)| (s, user)))
                .and_then(|(s, user)| s.create_merchant(user.id).map(|(s, _)| (s, user)))
                .and_then(move |(s, user)| {
                    // only if provider is email it needs to be verified
                    match provider {
                        Provider::Email => Box::new(s.notify_user(user.clone(), device, project).then(|res| match res {
                            Ok((s, _)) => Ok((s, user)),
                            Err((s, _)) => Ok((s, user)),
                        })) as ServiceFuture<Self, User>,
                        Provider::Facebook | Provider::Google if project.unwrap_or(Project::MarketPlace) == Project::MarketPlace => {
                            Box::new(
                                s.create_emarsys_contact(CreateEmarsysContactPayload {
                                    user_id: user.id,
                                    email: user.email.clone(),
                                }).then(|res| match res {
                                    Ok((s, _)) => Ok((s, user)),
                                    Err((s, _)) => Ok((s, user)),
                                }),
                            ) as ServiceFuture<Self, User>
                        }
                        _ => Box::new(future::ok((s, user))) as ServiceFuture<Self, User>,
                    }
                }),
        )
    }

    // Contains reversal of account creation
    fn create_revert(self) -> impl Future<Item = (Self, ()), Error = (Self, FailureError)> {
        let log = self.log.lock().unwrap().clone();

        let stores_microservice = self.stores_microservice.clone();
        let billing_microservice = self.billing_microservice.clone();
        let delivery_microservice = self.delivery_microservice.clone();
        let users_microservice = self.users_microservice.clone();

        let fut = iter_ok::<_, ()>(log).for_each(move |e| {
            match e {
                CreateProfileOperationStage::AccountCreationComplete(saga_id) => {
                    debug!("Reverting user, saga_id: {}", saga_id);
                    Box::new(
                        users_microservice
                            .delete_user(Some(Initiator::Superadmin), saga_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::UsersRoleSetComplete(role_id) => {
                    debug!("Reverting users role, role_id: {}", role_id);
                    let mut headers = Headers::new();
                    headers.set(Authorization("1".to_string())); // only super admin delete user role

                    Box::new(
                        users_microservice
                            .delete_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::StoreRoleSetComplete(role_id) => {
                    debug!("Reverting stores users role, role_id: {}", role_id);

                    Box::new(
                        stores_microservice
                            .delete_stores_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::BillingRoleSetComplete(role_id) => {
                    debug!("Reverting billing role, role_id: {}", role_id);

                    Box::new(
                        billing_microservice
                            .delete_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::DeliveryRoleSetComplete(role_id) => {
                    debug!("Reverting delivery role, role_id: {}", role_id);
                    Box::new(
                        delivery_microservice
                            .delete_delivery_role(Some(Initiator::Superadmin), role_id)
                            .then(|_| Ok(())),
                    ) as Box<Future<Item = (), Error = ()>>
                }

                CreateProfileOperationStage::BillingCreateMerchantComplete(user_id) => {
                    debug!("Reverting merchant, user_id: {}", user_id);
                    Box::new(
                        billing_microservice
                            .delete_user_merchant(Some(Initiator::Superadmin), user_id)
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
        let project_ = input.project.clone().unwrap_or_else(|| Project::MarketPlace);
        let reset_password_path = match project_ {
            Project::MarketPlace => {
                let config::DevicesUrls { web, ios, android } = self.config.notification_urls.reset_password.marketplace.clone();
                input
                    .device
                    .clone()
                    .map(|device| match device {
                        Device::WEB => web.clone(),
                        Device::IOS => ios,
                        Device::Android => android,
                    }).unwrap_or_else(|| web)
            }
            Project::Wallet => {
                let config::DevicesUrls { web, ios, android } = self.config.notification_urls.reset_password.wallet.clone();
                input
                    .device
                    .clone()
                    .map(|device| match device {
                        Device::WEB => web.clone(),
                        Device::IOS => ios,
                        Device::Android => android,
                    }).unwrap_or_else(|| web)
            }
        };

        let users_microservice = self.users_microservice.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let res = self
            .users_microservice
            .get_by_email(Some(Initiator::Superadmin), &input.email)
            .and_then(move |user| {
                if let Some(user) = user {
                    if user.is_blocked {
                        return Box::new(future::err(
                            Error::Validate(validation_errors!({"email": ["email" => "Email is blocked"]})).into(),
                        )) as Box<Future<Item = (), Error = FailureError>>;
                    }

                    let user_id = user.id;
                    Box::new(
                        users_microservice
                            .create_password_reset_token(Some(user_id.into()), input)
                            .and_then(move |token| {
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
                                notifications_microservice.password_reset(Some(Initiator::Superadmin), email, project_)
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
        let cluster_url = self.config.cluster.url.clone();

        let project_ = input.project.clone().unwrap_or_else(|| Project::MarketPlace);
        let users_microservice = self.users_microservice.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let res = self
            .users_microservice
            .apply_password_reset_token(Some(Initiator::Superadmin), input)
            .and_then(move |reset_token| {
                users_microservice
                    .get_by_email(Some(Initiator::Superadmin), &reset_token.email)
                    .map(|user| (user, reset_token.token))
            }).and_then(move |(user, token)| {
                if let Some(user) = user {
                    let user = EmailUser {
                        email: user.email.clone(),
                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                    };
                    let email = ApplyPasswordResetForUser { user, cluster_url };
                    Box::new(
                        notifications_microservice
                            .apply_password_reset(Some(Initiator::Superadmin), email, project_)
                            .map(|_| token),
                    )
                } else {
                    Box::new(future::err(
                        Error::Validate(validation_errors!({"email": ["email" => "Email does not exists"]})).into(),
                    )) as Box<Future<Item = String, Error = FailureError>>
                }
            }).then(|res| match res {
                Ok(token) => Ok((Box::new(self) as Box<AccountService>, token)),
                Err(e) => Err((Box::new(self) as Box<AccountService>, e)),
            });
        Box::new(res)
    }

    fn request_email_verification(self, input: VerifyRequest) -> ServiceFuture<Box<AccountService>, ()> {
        let project_ = input.project.clone().unwrap_or_else(|| Project::MarketPlace);
        let verify_email_path = match project_ {
            Project::MarketPlace => {
                let config::DevicesUrls { web, ios, android } = self.config.notification_urls.verify_email.marketplace.clone();
                input
                    .device
                    .clone()
                    .map(|device| match device {
                        Device::WEB => web.clone(),
                        Device::IOS => ios,
                        Device::Android => android,
                    }).unwrap_or_else(|| web)
            }
            Project::Wallet => {
                let config::DevicesUrls { web, ios, android } = self.config.notification_urls.verify_email.wallet.clone();
                input
                    .device
                    .clone()
                    .map(|device| match device {
                        Device::WEB => web.clone(),
                        Device::IOS => ios,
                        Device::Android => android,
                    }).unwrap_or_else(|| web)
            }
        };

        let users_microservice = self.users_microservice.clone();
        let notifications_microservice = self.notifications_microservice.clone();
        let res = self
            .users_microservice
            .get_by_email(Some(Initiator::Superadmin), &input.email)
            .and_then(move |user| {
                if let Some(user) = user {
                    if user.is_blocked {
                        return Box::new(future::err(
                            Error::Validate(validation_errors!({"email": ["email" => "Email is blocked"]})).into(),
                        )) as Box<Future<Item = (), Error = FailureError>>;
                    }

                    Box::new(
                        users_microservice
                            .create_email_verify_token(Some(Initiator::Superadmin), input)
                            .and_then(move |token| {
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
                                notifications_microservice.email_verification(Some(Initiator::Superadmin), email, project_)
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
        let notifications_microservice = self.notifications_microservice.clone();
        let users_microservice = self.users_microservice.clone();
        let project_ = input.project.clone().unwrap_or_else(|| Project::MarketPlace);
        Box::new(
            users_microservice
                .apply_email_verify_token(Some(Initiator::Superadmin), input)
                .and_then(move |email_apply_token| {
                    let EmailVerifyApplyToken { user, token } = email_apply_token;
                    let user_id = user.id;
                    let user_email = user.email.clone();
                    let email_user = EmailUser {
                        email: user.email.clone(),
                        first_name: user.first_name.unwrap_or_else(|| "user".to_string()),
                        last_name: user.last_name.unwrap_or_else(|| "".to_string()),
                    };
                    let email = ApplyEmailVerificationForUser { user: email_user };

                    notifications_microservice
                        .apply_email_verification(Some(Initiator::Superadmin), email, project_)
                        .map(move |_| (user_id, user_email, token))
                }).then(|res| match res {
                    Ok((user_id, email, token)) => Ok((self, user_id, email, token)),
                    Err(err) => Err((self, err)),
                }).and_then(move |(self_service, user_id, email, token)| {
                    self_service
                        .create_emarsys_contact(CreateEmarsysContactPayload { user_id, email })
                        .then(|res| match res {
                            Ok((self_service, _)) => Ok((self_service, token)),
                            Err((self_service, _)) => Ok((self_service, token)),
                        })
                }).then(|res| match res {
                    Ok((self_service, token)) => Ok((Box::new(self_service) as Box<AccountService>, token)),
                    Err((self_service, e)) => Err((Box::new(self_service) as Box<AccountService>, e)),
                }),
        )
    }
}

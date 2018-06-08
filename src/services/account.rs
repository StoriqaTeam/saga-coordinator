use std::sync::{Arc, Mutex};

use futures;
use futures::prelude::*;
use hyper::Method;
use serde_json;
use uuid::Uuid;

use stq_http;
use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::role::UserRole as StqUserRole;
use stq_routes::service::Service as StqService;

use config;
use models::*;
use services::types::ServiceFuture;


pub trait AccountService {
    fn create(self, input: SagaCreateProfile) -> ServiceFuture<Option<User>>;
}

/// Attributes services, responsible for Attribute-related CRUD operations
pub struct AccountServiceImpl {
    pub http_client: Arc<HttpClientHandle>,
    pub config: config::Config,
    pub log : Arc<Mutex<OperationLog>>,
}

impl AccountServiceImpl
{
    pub fn new(http_client: Arc<HttpClientHandle>, config: config::Config) -> Self {
        let log = Arc::new(Mutex::new(OperationLog::new()));
        Self {
            http_client,
            config,
            log
        }
    }

    pub fn create_user(&self, input: SagaCreateProfile, saga_id_arg: String) -> ServiceFuture<User> {
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
            birthdate: input_user.birthdate.clone(),
            last_login_at: input_user.last_login_at.clone(),
            saga_id: saga_id_arg.clone(),
        });
        let create_profile = SagaCreateProfile {
            user: new_user,
            identity: new_ident,
        };

        let body = serde_json::to_string(&create_profile).unwrap();
        self.log.lock().unwrap().push(OperationStage::AccountCreationStart(saga_id_arg.clone()));

        let res = self.http_client
            .request::<User>(
                Method::Post,
                format!("{}/{}", self.config.service_url(StqService::Users), StqModel::User.to_url()),
                Some(body),
                None,
            )
            .and_then(move |v| {
                self.log.lock()
                    .unwrap()
                    .push(OperationStage::AccountCreationComplete(saga_id_arg.clone()));
                futures::future::ok(v)
            });

        Box::new(res)
    }

    fn create_user_role(&self,user_id: i32) -> ServiceFuture<StqUserRole> { 
        // Create account
        self.log.lock().unwrap().push(OperationStage::UsersRoleSetStart(user_id.clone()));

        let res = self.http_client.request::<StqUserRole>(
            Method::Post,
            format!("{}/{}/{}", self.config.service_url(StqService::Users), "roles/default", user_id.clone()),
            None,
            None,
        );

        self.log.lock().unwrap().push(OperationStage::UsersRoleSetComplete(user_id.clone()));

        Box::new(res)
    }

    fn create_store_role(&self,user_id: i32) -> ServiceFuture<StqUserRole> {
        // Create account
        self.log.lock().unwrap().push(OperationStage::StoreRoleSetStart(user_id.clone()));

        let res = self.http_client.request::<StqUserRole>(
            Method::Post,
            format!("{}/{}/{}", self.config.service_url(StqService::Stores), "roles/default", user_id.clone()),
            None,
            None,
        );

        self.log.lock().unwrap().push(OperationStage::StoreRoleSetComplete(user_id.clone()));

        Box::new(res)
    }

    // Contains happy path for account creation
    fn create_happy(&self,input: SagaCreateProfile) -> ServiceFuture<User> {
        let saga_id = Uuid::new_v4().to_string();

        Box::new(
            self.create_user(input.clone(), saga_id.clone()).and_then({
                move |user| {
                    self.create_user_role(user.id.clone())
                        .map(|_v| user)
                        .and_then({ move |user| self.create_store_role(user.id).map(|_v| user) })
                }
            }),
        )
    }

    // Contains reversal of account creation
    fn create_revert(&self) -> ServiceFuture<()> {
        let mut fut: ServiceFuture<()> = Box::new(futures::future::ok(()));
        for e in operation_log {
            match e {
                OperationStage::StoreRoleSetStart(user_id) => {
                    println!("Reverting users role, user_id: {}", user_id);
                    fut = Box::new(fut.and_then({
                        move |_r| {
                            self.http_client.request::<StqUserRole>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    self.config.service_url(StqService::Stores),
                                    //StqModel::UserRoles.to_url(),
                                    "roles/default",
                                    user_id.clone(),
                                ),
                                None,
                                None,
                            )
                        }
                    }).map(|_v| ()));
                }

                OperationStage::AccountCreationStart(saga_id) => {
                    println!("Reverting user, saga_id: {}", saga_id);
                    fut = Box::new(fut.and_then({
                        move |_res| {
                            self.http_client.request::<StqUserRole>(
                                Method::Delete,
                                format!(
                                    "{}/{}/{}",
                                    config.service_url(StqService::Users),
                                    //StqModel::UserRoles.to_url(),
                                    "user_by_saga_id",
                                    saga_id.clone(),
                                ),
                                None,
                                None,
                            )
                        }
                    }).map(|_v| ()));
                }

                _ => {}
            }
        }

        fut
    }
}

impl AccountService for AccountServiceImpl {
    pub fn create(self, input: SagaCreateProfile) -> ServiceFuture<Option<User>> {
        Box::new(
            self.create_happy(input.clone())
                .map(|user| Some(user))
                .or_else(move |e| {
                    // Arc::try_unwrap(log).unwrap().into_inner().unwrap(),
                        self.create_revert()
                            .then(move |_res| futures::future::err(e.context("Service Account, create endpoint error occured.")))
                    }
                )
        )
    }
}





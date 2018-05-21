use config;
use chrono::NaiveDate;
use futures;
use futures::prelude::*;
use hyper::Method;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use serde_json;
use stq_http;
use stq_http::client::ClientHandle as HttpClientHandle;
use stq_routes::model::Model as StqModel;
use stq_routes::role::UserRole as StqUserRole;
use stq_routes::service::Service as StqService;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Gender {
    Male,
    Female,
    Undefined,
}

impl FromStr for Gender {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Male" => Ok(Gender::Male),
            "Female" => Ok(Gender::Female),
            _ => Ok(Gender::Undefined),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub email_verified: bool,
    pub phone: Option<String>,
    pub phone_verified: bool,
    pub is_active: bool,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub gender: Gender,
    pub birthdate: Option<NaiveDate>,
    pub last_login_at: SystemTime,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub saga_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewUser {
    pub email: String,
    pub phone: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub gender: Gender,
    pub birthdate: Option<NaiveDate>,
    pub last_login_at: SystemTime,
    pub saga_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Provider {
    Google,
    Facebook,
    Email,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewIdentity {
    pub email: String,
    pub password: Option<String>,
    pub provider: Provider,
    pub saga_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SagaCreateProfile {
    pub user: Option<NewUser>,
    pub identity: NewIdentity,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub enum Role {
    Superuser,
    User,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct NewUserRole {
    pub user_id: i32,
    pub role: Role,
}

#[derive(Deserialize, Debug)]
pub struct UserRole {
    pub id: i32,
    pub saga_id: String,
    pub user_id: i32,
    pub role: Role,
}

pub type OperationLog = Vec<OperationStage>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum OperationStage {
    AccountCreationStart(String),
    AccountCreationComplete(String),
    UsersRoleSetStart(i32),
    UsersRoleSetComplete(i32),
    StoreRoleSetStart(i32),
    StoreRoleSetComplete(i32),
}

fn create_user(
    http_client: Arc<HttpClientHandle>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    input: SagaCreateProfile,
    saga_id_arg: String,
) -> Box<Future<Item = User, Error = stq_http::client::Error>> {
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
    log.lock()
        .unwrap()
        .push(OperationStage::AccountCreationStart(saga_id_arg.clone()));

    let res = http_client
        .request::<User>(
            Method::Post,
            format!(
                "{}/{}",
                config.service_url(StqService::Users),
                StqModel::User.to_url()
            ),
            Some(body),
            None,
        )
        .and_then(move |v| {
            log.lock()
                .unwrap()
                .push(OperationStage::AccountCreationComplete(saga_id_arg.clone()));
            futures::future::ok(v)
        });

    Box::new(res)
}

fn create_user_role(
    http_client: Arc<HttpClientHandle>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    user_id: i32,
) -> Box<Future<Item = StqUserRole, Error = stq_http::client::Error>> {
    // Create account
    log.lock()
        .unwrap()
        .push(OperationStage::UsersRoleSetStart(user_id.clone()));

    let res = http_client.request::<StqUserRole>(
        Method::Post,
        format!(
            "{}/{}/{}",
            config.service_url(StqService::Users),
            "roles/default",
            user_id.clone()
        ),
        None,
        None,
    );

    log.lock()
        .unwrap()
        .push(OperationStage::UsersRoleSetComplete(user_id.clone()));

    Box::new(res)
}

fn create_store_role(
    http_client: Arc<HttpClientHandle>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    user_id: i32,
) -> Box<Future<Item = StqUserRole, Error = stq_http::client::Error>> {
    // Create account
    log.lock()
        .unwrap()
        .push(OperationStage::StoreRoleSetStart(user_id.clone()));

    let res = http_client.request::<StqUserRole>(
        Method::Post,
        format!(
            "{}/{}/{}",
            config.service_url(StqService::Stores),
            "roles/default",
            user_id.clone()
        ),
        None,
        None,
    );

    log.lock()
        .unwrap()
        .push(OperationStage::StoreRoleSetComplete(user_id.clone()));

    Box::new(res)
}

// Contains happy path for account creation
fn create_happy(
    http_client: Arc<HttpClientHandle>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    input: SagaCreateProfile,
) -> Box<Future<Item = User, Error = stq_http::client::Error>> {
    let saga_id = Uuid::new_v4().to_string();

    Box::new(
        create_user(
            http_client.clone(),
            log.clone(),
            config.clone(),
            input.clone(),
            saga_id.clone(),
        ).and_then({
            let http_client = http_client.clone();
            let log = log.clone();
            let config = config.clone();

            let http_client2 = http_client.clone();
            let log2 = log.clone();
            let config2 = config.clone();

            move |user| {
                create_user_role(http_client, log, config, user.id.clone())
                    .map(|_v| user)
                    .and_then({ move |user| create_store_role(http_client2, log2, config2, user.id).map(|_v| user) })
            }
        }),
    )
}

// Contains reversal of account creation
fn create_revert(
    http_client: Arc<HttpClientHandle>,
    operation_log: OperationLog,
    config: config::Config,
) -> Box<Future<Item = (), Error = stq_http::client::Error>> {
    let mut fut: Box<Future<Item = (), Error = stq_http::client::Error>> = Box::new(futures::future::ok(()));
    for e in operation_log {
        match e {
            OperationStage::StoreRoleSetStart(user_id) => {
                println!("Reverting users role, user_id: {}", user_id);
                fut = Box::new(fut.and_then({
                    let config = config.clone();
                    let http_client = http_client.clone();
                    move |_r| {
                        http_client.request::<StqUserRole>(
                            Method::Delete,
                            format!(
                                "{}/{}/{}",
                                config.service_url(StqService::Stores),
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
                    let config = config.clone();
                    let http_client = http_client.clone();
                    move |_res| {
                        http_client.request::<StqUserRole>(
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

pub fn create(
    http_client: Arc<HttpClientHandle>,
    config: config::Config,
    body: String,
) -> Box<Future<Item = Option<User>, Error = stq_http::client::Error>> {
    let config2 = config.clone();
    let log = Arc::new(Mutex::new(OperationLog::new()));

    Box::new(
        serde_json::from_str::<SagaCreateProfile>(&body)
            .into_future()
            .map_err(|_e| stq_http::client::Error::Parse("Deserialization fail".to_string()))
            .and_then({
                let http_client = http_client.clone();
                move |input| {
                    create_happy(
                        http_client.clone(),
                        log.clone(),
                        config.clone(),
                        input.clone(),
                    ).map(|user| Some(user))
                        .or_else({
                            let http_client = http_client.clone();
                            move |e| {
                                create_revert(
                                    http_client,
                                    Arc::try_unwrap(log).unwrap().into_inner().unwrap(),
                                    config2,
                                ).then(move |_res| futures::future::err(e))
                            }
                        })
                }
            }),
    )
}

use config;

use failure;
use futures::prelude::*;
use hyper::Method;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use serde_json;
use stq_http;
use stq_http::client::Client as HttpClient;
use stq_routes::model::Model as StqModel;
use stq_routes::role::Role as StqRole;
use stq_routes::role::UserRole as StqUserRole;
use stq_routes::role::NewUserRole as StqNewUserRole;
use stq_routes::service::Service as StqService;
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
    pub saga_id: String,
    pub email: String,
    pub is_active: bool,
    pub phone: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub gender: Gender,
    pub birthdate: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateUserInput {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct NewIdentity {
    pub saga_id: String,
    pub email: String,
    pub password: String,
}

impl From<(String, CreateUserInput)> for NewIdentity {
    fn from(v: (String, CreateUserInput)) -> NewIdentity {
        Self {
            saga_id: v.0,
            email: v.1.email,
            password: v.1.password,
        }
    }
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
    http_client: Arc<HttpClient>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    input: CreateUserInput,
    saga_id: String,
) -> Box<Future<Item = User, Error = failure::Error>> {
    // Create account
    let v = NewIdentity::from((saga_id.clone(), input));
    let body = serde_json::to_string(&v).unwrap();
    log.lock()
        .unwrap()
        .push(OperationStage::AccountCreationStart(saga_id.clone()));

    let res = http_client.handle().request::<User>(
        Method::Post,
        format!(
            "{}/{}",
            config.service_url(StqService::Users),
            StqModel::User.to_url()
        ),
        Some(body),
        None
    ).map_err(|e| format_err!(""));

    log.lock()
        .unwrap()
        .push(OperationStage::AccountCreationComplete(saga_id.clone()));

    Box::new(res)
}

fn create_user_role(
    http_client: Arc<HttpClient>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    input: CreateUserInput,
    user_id: i32,
) -> Box<Future<Item = StqUserRole, Error = failure::Error>> {
    // Create account
    log.lock()
        .unwrap()
        .push(OperationStage::UsersRoleSetStart(user_id.clone()));

    let res = http_client.handle().request::<StqUserRole>(
        Method::Post,
        format!(
            "{}/{}/{}",
            config.service_url(StqService::Users),
            "roles/default",
            user_id.clone()
        ),
        None,
        None
    ).map_err(|e| format_err!(""));

    log.lock()
        .unwrap()
        .push(OperationStage::UsersRoleSetComplete(user_id.clone()));

    Box::new(res)
}

fn create_store_role(
    http_client: Arc<HttpClient>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    input: CreateUserInput,
    user_id: i32,
) -> Box<Future<Item = StqUserRole, Error = failure::Error>> {
    // Create account
    log.lock()
        .unwrap()
        .push(OperationStage::StoreRoleSetStart(user_id.clone()));

    let res = http_client.handle().request::<StqUserRole>(
        Method::Post,
        format!(
            "{}/{}/{}",
            config.service_url(StqService::Stores),
            "roles/default",
            user_id.clone()
        ),
        None,
        None
    ).map_err(|e| format_err!(""));

    log.lock()
        .unwrap()
        .push(OperationStage::StoreRoleSetComplete(user_id.clone()));

    Box::new(res)
}

// Contains happy path for account creation
fn create_happy(
    http_client: Arc<HttpClient>,
    log: Arc<Mutex<OperationLog>>,
    config: config::Config,
    input: CreateUserInput,
) -> Box<Future<Item = (), Error = failure::Error>> {

    let http_client2 = http_client.clone();
    let log2 = log.clone();
    let config2 = config.clone();
    let input2 = input.clone();

    let http_client3 = http_client.clone();
    let log3 = log.clone();
    let config3 = config.clone();
    let input3 = input.clone();

    let saga_id = Uuid::new_v4().to_string();

    Box::new(
        create_user(
            http_client.clone(),
            log.clone(),
            config.clone(),
            input.clone(),
            saga_id.clone(),
        )
        .and_then(|user| {
            create_user_role(
                http_client2,
                log2,
                config2,
                input2,
                user.id.clone(),
            )
            .and_then(|user_role| {
                create_store_role(
                    http_client3,
                    log3,
                    config3,
                    input3,
                    user_role.user_id,
                )
            })
        }).map(|user_role| ())
    )
}

// Contains reversal of account creation
fn create_revert(
    http_client: Arc<HttpClient>,
    operation_log: OperationLog,
    config: config::Config
) -> Result<(), failure::Error> {
    for e in operation_log {
        match e {
            OperationStage::StoreRoleSetStart(user_id) => {
                http_client.handle().request::<StqUserRole>(
                    Method::Delete,
                    format!(
                        "{}/{}/{}",
                        config.service_url(StqService::Stores),
                        //StqModel::UserRoles.to_url(),
                        "roles/default",
                        user_id.clone(),
                    ),
                    None,
                    None
                );
            },

            OperationStage::AccountCreationStart(saga_id) => {
                http_client.handle().request::<StqUserRole>(
                    Method::Delete,
                    format!(
                        "{}/{}/{}",
                        config.service_url(StqService::Users),
                        //StqModel::UserRoles.to_url(),
                        "users_by_saga_id",
                        saga_id.clone(),
                    ),
                    None,
                    None
                );
            },

            _ => {}
        }
    }

    Ok(())
}


pub fn create(
    http_client: Arc<HttpClient>,
    config: config::Config,
    body: String
) -> Box<Future<Item = String, Error = failure::Error>> {

    let http_client2 = http_client.clone();
    let config2 = config.clone();

    let input = serde_json::from_str::<CreateUserInput>(&body).unwrap();

    let log = Arc::new(Mutex::new(OperationLog::new()));

    Box::new(
        create_happy(
            http_client.clone(),
            log.clone(),
            config.clone(),
            input.clone(),
        ).map_err(|e| {
            let rev_res = create_revert(
                http_client2,
                Arc::try_unwrap(log).unwrap().into_inner().unwrap(),
                config2,
            );

            match rev_res {
                Ok(_) => format_err!("Revert Ok"),
                Err(_) => format_err!("Revert Err"),
            }
        })
        .map(|res| "Ok".to_string())
    )
}

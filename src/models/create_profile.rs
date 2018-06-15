use chrono::NaiveDate;
use std::str::FromStr;
use std::time::SystemTime;

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

pub type CreateProfileOperationLog = Vec<CreateProfileOperationStage>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CreateProfileOperationStage {
    AccountCreationStart(String),
    AccountCreationComplete(String),
    UsersRoleSetStart(i32),
    UsersRoleSetComplete(i32),
    StoreRoleSetStart(i32),
    StoreRoleSetComplete(i32),
}

use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use chrono::NaiveDate;

use stq_types::{MerchantId, RoleEntryId, SagaId, UserId, UsersRole};

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
    pub id: UserId,
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
    pub saga_id: SagaId,
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
    pub saga_id: SagaId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub saga_id: SagaId,
}

impl fmt::Display for NewIdentity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "NewIdentity: 
        email: {},
        password: '****',
        provider: {:?},
        saga_id: {}",
            self.email, self.provider, self.saga_id,
        )
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SagaCreateProfile {
    pub user: Option<NewUser>,
    pub identity: NewIdentity,
}

impl fmt::Display for SagaCreateProfile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SagaCreateProfile - user: {:#?}, identity: {})", self.user, self.identity)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct NewUserRole {
    pub user_id: UserId,
    pub role: UsersRole,
}

#[derive(Deserialize, Debug)]
pub struct UserRole {
    pub id: i32,
    pub user_id: UserId,
    pub role: UsersRole,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateUserMerchantPayload {
    pub id: UserId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Merchant {
    pub merchant_id: MerchantId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResetRequest {
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EmailVerifyApply {
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PasswordResetApply {
    pub token: String,
    pub password: String,
}

pub type CreateProfileOperationLog = Vec<CreateProfileOperationStage>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CreateProfileOperationStage {
    AccountCreationStart(SagaId),
    AccountCreationComplete(SagaId),
    UsersRoleSetStart(UserId),
    UsersRoleSetComplete(UserId),
    StoreRoleSetStart(UserId),
    StoreRoleSetComplete(UserId),
    BillingRoleSetStart(RoleEntryId),
    BillingRoleSetComplete(RoleEntryId),
    BillingCreateMerchantStart(UserId),
    BillingCreateMerchantComplete(UserId),
}

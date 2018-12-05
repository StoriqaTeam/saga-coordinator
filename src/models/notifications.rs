use stq_types::{Alpha3, EmarsysId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmarsysContactPayload {
    pub user_id: UserId,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub country: Option<Alpha3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedEmarsysContact {
    pub user_id: UserId,
    pub emarsys_id: EmarsysId,
}

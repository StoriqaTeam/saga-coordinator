use stq_types::{EmarsysId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmarsysContactPayload {
    pub user_id: UserId,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedEmarsysContact {
    pub user_id: UserId,
    pub emarsys_id: EmarsysId,
}

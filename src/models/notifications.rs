use stq_types::{EmarsysId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmarsysContactPayload {
    pub user_id: UserId,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedEmarsysContact {
    pub user_id: UserId,
    pub emarsys_id: EmarsysId,
}

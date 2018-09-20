use stq_types::{OrderRole, RoleEntryId, RoleId, StoreId, UserId, WarehouseRole};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewRole<Role> {
    pub id: RoleId,
    pub user_id: UserId,
    pub name: Role,
    pub data: Option<StoreId>,
}

impl<Role> NewRole<Role> {
    pub fn new(id: RoleId, user_id: UserId, name: Role, data: Option<StoreId>) -> Self {
        Self { id, user_id, name, data }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoleEntry<Payload> {
    pub id: RoleEntryId,
    pub user_id: UserId,
    pub role: Payload,
}

impl<Payload> RoleEntry<Payload> {
    pub fn new(id: RoleEntryId, user_id: UserId, role: Payload) -> Self {
        Self { id, user_id, role }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewWarehouseRole {
    pub name: WarehouseRole,
    pub data: StoreId,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NewOrdersRole {
    pub name: OrderRole,
    pub data: StoreId,
}

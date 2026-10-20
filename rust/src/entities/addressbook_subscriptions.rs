use crate::db::id::{Id, Int};
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "addressbook_subscriptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub vault_id: Uuid,
    pub uri: String,
    pub username: String,
    pub password: String,
    pub active: bool,
    pub sync_way: Int,
    pub capabilities: String,
    pub distant_sync_token: Option<String>,
    pub last_batch: Option<String>,
    pub current_logid: Option<Id>,
    pub sync_token_id: Option<Id>,
    pub frequency: i16,
    pub last_synchronized_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::job_batches::Entity",
        from = "Column::LastBatch",
        to = "super::job_batches::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    JobBatches,
    #[sea_orm(
        belongs_to = "super::sync_tokens::Entity",
        from = "Column::SyncTokenId",
        to = "super::sync_tokens::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    SyncTokens,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Users,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::job_batches::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::JobBatches.def()
    }
}

impl Related<super::sync_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncTokens.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

timestamps!();

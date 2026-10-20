use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "personal_access_tokens")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub tokenable_type: String,
    pub tokenable_id: Uuid,
    pub name: String,
    #[sea_orm(unique)]
    pub token: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub abilities: Option<String>,
    pub last_used_at: Option<Timestamp>,
    pub expires_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

timestamps!();

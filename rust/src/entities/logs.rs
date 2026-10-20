use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub group_id: Id,
    pub level: String,
    pub level_name: String,
    pub channel: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub message: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub context: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub extra: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub formatted: Option<String>,
    pub loggable_type: String,
    pub loggable_id: Uuid,
    pub logged_at: Timestamp,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

timestamps!();

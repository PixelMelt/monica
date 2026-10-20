use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "journals")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub vault_id: Uuid,
    pub name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::journal_metrics::Entity")]
    JournalMetrics,
    #[sea_orm(has_many = "super::posts::Entity")]
    Posts,
    #[sea_orm(has_many = "super::slice_of_lives::Entity")]
    SliceOfLives,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::journal_metrics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::JournalMetrics.def()
    }
}

impl Related<super::posts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Posts.def()
    }
}

impl Related<super::slice_of_lives::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SliceOfLives.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

timestamps!();

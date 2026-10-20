use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "journal_metrics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub journal_id: Id,
    pub label: String,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::journals::Entity",
        from = "Column::JournalId",
        to = "super::journals::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Journals,
    #[sea_orm(has_many = "super::post_metrics::Entity")]
    PostMetrics,
}

impl Related<super::journals::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Journals.def()
    }
}

impl Related<super::post_metrics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostMetrics.def()
    }
}

timestamps!();

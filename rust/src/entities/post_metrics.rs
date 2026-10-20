use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "post_metrics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub post_id: Id,
    pub journal_metric_id: Id,
    pub value: i32,
    pub label: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::journal_metrics::Entity",
        from = "Column::JournalMetricId",
        to = "super::journal_metrics::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    JournalMetrics,
    #[sea_orm(
        belongs_to = "super::posts::Entity",
        from = "Column::PostId",
        to = "super::posts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Posts,
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

timestamps!();

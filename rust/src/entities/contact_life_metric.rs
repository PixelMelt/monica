use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_life_metric")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub life_metric_id: Id,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
    #[sea_orm(
        belongs_to = "super::life_metrics::Entity",
        from = "Column::LifeMetricId",
        to = "super::life_metrics::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    LifeMetrics,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::life_metrics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeMetrics.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

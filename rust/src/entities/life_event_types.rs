use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "life_event_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub life_event_category_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub can_be_deleted: bool,
    pub position: Option<i32>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::life_event_categories::Entity",
        from = "Column::LifeEventCategoryId",
        to = "super::life_event_categories::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    LifeEventCategories,
    #[sea_orm(has_many = "super::life_events::Entity")]
    LifeEvents,
}

impl Related<super::life_event_categories::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEventCategories.def()
    }
}

impl Related<super::life_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEvents.def()
    }
}

timestamps!();

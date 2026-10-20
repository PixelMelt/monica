use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "life_event_participants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub life_event_id: Id,
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
        belongs_to = "super::life_events::Entity",
        from = "Column::LifeEventId",
        to = "super::life_events::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    LifeEvents,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::life_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEvents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "timeline_event_participants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub timeline_event_id: Id,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
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
        belongs_to = "super::timeline_events::Entity",
        from = "Column::TimelineEventId",
        to = "super::timeline_events::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    TimelineEvents,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::timeline_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TimelineEvents.def()
    }
}

timestamps!();

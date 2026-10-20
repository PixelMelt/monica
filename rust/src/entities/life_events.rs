use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "life_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub timeline_event_id: Id,
    pub life_event_type_id: Id,
    pub emotion_id: Option<Id>,
    pub happened_at: Date,
    pub collapsed: bool,
    pub summary: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub costs: Option<i32>,
    pub currency_id: Option<Id>,
    pub paid_by_contact_id: Option<Uuid>,
    pub duration_in_minutes: Option<i32>,
    pub distance: Option<i32>,
    pub distance_unit: Option<String>,
    pub from_place: Option<String>,
    pub to_place: Option<String>,
    pub place: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::PaidByContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Contacts,
    #[sea_orm(
        belongs_to = "super::currencies::Entity",
        from = "Column::CurrencyId",
        to = "super::currencies::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Currencies,
    #[sea_orm(
        belongs_to = "super::emotions::Entity",
        from = "Column::EmotionId",
        to = "super::emotions::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Emotions,
    #[sea_orm(has_many = "super::life_event_participants::Entity")]
    LifeEventParticipants,
    #[sea_orm(
        belongs_to = "super::life_event_types::Entity",
        from = "Column::LifeEventTypeId",
        to = "super::life_event_types::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    LifeEventTypes,
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

impl Related<super::currencies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Currencies.def()
    }
}

impl Related<super::emotions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Emotions.def()
    }
}

impl Related<super::life_event_participants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEventParticipants.def()
    }
}

impl Related<super::life_event_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEventTypes.def()
    }
}

impl Related<super::timeline_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TimelineEvents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

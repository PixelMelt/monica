use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_reminders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub label: String,
    pub day: Option<i32>,
    pub month: Option<i32>,
    pub year: Option<i32>,
    pub r#type: String,
    pub frequency_number: Option<i32>,
    pub last_triggered_at: Option<DateTime>,
    pub number_times_triggered: i32,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contact_reminder_scheduled::Entity")]
    ContactReminderScheduled,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
}

impl Related<super::contact_reminder_scheduled::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactReminderScheduled.def()
    }
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

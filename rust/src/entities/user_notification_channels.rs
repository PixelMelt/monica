use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "user_notification_channels")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub user_id: Option<Uuid>,
    pub r#type: String,
    pub label: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub preferred_time: Option<Time>,
    pub active: bool,
    pub verified_at: Option<DateTime>,
    pub verification_token: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub fails: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contact_reminder_scheduled::Entity")]
    ContactReminderScheduled,
    #[sea_orm(has_many = "super::user_notification_sent::Entity")]
    UserNotificationSent,
}

impl Related<super::contact_reminder_scheduled::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactReminderScheduled.def()
    }
}

impl Related<super::user_notification_sent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserNotificationSent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

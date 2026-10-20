use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_reminder_scheduled")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub user_notification_channel_id: Id,
    pub contact_reminder_id: Id,
    pub scheduled_at: Timestamp,
    pub triggered_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contact_reminders::Entity",
        from = "Column::ContactReminderId",
        to = "super::contact_reminders::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ContactReminders,
    #[sea_orm(
        belongs_to = "super::user_notification_channels::Entity",
        from = "Column::UserNotificationChannelId",
        to = "super::user_notification_channels::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    UserNotificationChannels,
}

impl Related<super::contact_reminders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactReminders.def()
    }
}

impl Related<super::user_notification_channels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserNotificationChannels.def()
    }
}

timestamps!();

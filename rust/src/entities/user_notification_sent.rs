use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "user_notification_sent")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub user_notification_channel_id: Id,
    pub sent_at: DateTime,
    pub subject_line: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub payload: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    #[sea_orm(column_type = "Text", nullable)]
    pub error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user_notification_channels::Entity",
        from = "Column::UserNotificationChannelId",
        to = "super::user_notification_channels::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    UserNotificationChannels,
}

impl Related<super::user_notification_channels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserNotificationChannels.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

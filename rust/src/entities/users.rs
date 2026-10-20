use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub account_id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub password: Option<String>,
    #[sea_orm(unique)]
    pub email: String,
    pub email_verified_at: Option<Timestamp>,
    pub name_order: String,
    pub date_format: String,
    pub timezone: Option<String>,
    pub number_format: String,
    pub default_map_site: String,
    pub distance_format: String,
    pub is_account_administrator: bool,
    pub help_shown: bool,
    pub invitation_code: Option<String>,
    pub invitation_accepted_at: Option<Timestamp>,
    pub locale: String,
    pub remember_token: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    #[sea_orm(column_type = "Text", nullable)]
    pub two_factor_secret: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub two_factor_recovery_codes: Option<String>,
    pub two_factor_confirmed_at: Option<Timestamp>,
    pub contact_sort_order: String,
    pub is_instance_administrator: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Accounts,
    #[sea_orm(has_many = "super::addressbook_subscriptions::Entity")]
    AddressbookSubscriptions,
    #[sea_orm(has_many = "super::calls::Entity")]
    Calls,
    #[sea_orm(has_many = "super::contact_feed_items::Entity")]
    ContactFeedItems,
    #[sea_orm(has_many = "super::contact_tasks::Entity")]
    ContactTasks,
    #[sea_orm(has_many = "super::contact_vault_user::Entity")]
    ContactVaultUser,
    #[sea_orm(has_many = "super::notes::Entity")]
    Notes,
    #[sea_orm(has_many = "super::sync_tokens::Entity")]
    SyncTokens,
    #[sea_orm(has_many = "super::user_vault::Entity")]
    UserVault,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Accounts.def()
    }
}

impl Related<super::addressbook_subscriptions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AddressbookSubscriptions.def()
    }
}

impl Related<super::calls::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Calls.def()
    }
}

impl Related<super::contact_feed_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactFeedItems.def()
    }
}

impl Related<super::contact_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactTasks.def()
    }
}

impl Related<super::contact_vault_user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactVaultUser.def()
    }
}

impl Related<super::notes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Notes.def()
    }
}

impl Related<super::sync_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncTokens.def()
    }
}

impl Related<super::user_vault::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserVault.def()
    }
}

timestamps!();

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "vaults")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub account_id: Uuid,
    pub r#type: String,
    pub name: String,
    pub description: Option<String>,
    pub default_template_id: Option<Id>,
    pub default_activity_tab: String,
    pub show_group_tab: bool,
    pub show_tasks_tab: bool,
    pub show_files_tab: bool,
    pub show_journal_tab: bool,
    pub show_companies_tab: bool,
    pub show_reports_tab: bool,
    pub show_calendar_tab: bool,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
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
    #[sea_orm(has_many = "super::addresses::Entity")]
    Addresses,
    #[sea_orm(has_many = "super::companies::Entity")]
    Companies,
    #[sea_orm(has_many = "super::contact_important_date_types::Entity")]
    ContactImportantDateTypes,
    #[sea_orm(has_many = "super::contact_vault_user::Entity")]
    ContactVaultUser,
    #[sea_orm(has_many = "super::contacts::Entity")]
    Contacts,
    #[sea_orm(has_many = "super::files::Entity")]
    Files,
    #[sea_orm(has_many = "super::groups::Entity")]
    Groups,
    #[sea_orm(has_many = "super::journals::Entity")]
    Journals,
    #[sea_orm(has_many = "super::labels::Entity")]
    Labels,
    #[sea_orm(has_many = "super::life_event_categories::Entity")]
    LifeEventCategories,
    #[sea_orm(has_many = "super::life_metrics::Entity")]
    LifeMetrics,
    #[sea_orm(has_many = "super::loans::Entity")]
    Loans,
    #[sea_orm(has_many = "super::mood_tracking_parameters::Entity")]
    MoodTrackingParameters,
    #[sea_orm(has_many = "super::notes::Entity")]
    Notes,
    #[sea_orm(has_many = "super::tags::Entity")]
    Tags,
    #[sea_orm(
        belongs_to = "super::templates::Entity",
        from = "Column::DefaultTemplateId",
        to = "super::templates::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Templates,
    #[sea_orm(has_many = "super::timeline_events::Entity")]
    TimelineEvents,
    #[sea_orm(has_many = "super::user_vault::Entity")]
    UserVault,
    #[sea_orm(has_many = "super::vault_quick_facts_templates::Entity")]
    VaultQuickFactsTemplates,
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

impl Related<super::addresses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Addresses.def()
    }
}

impl Related<super::companies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Companies.def()
    }
}

impl Related<super::contact_important_date_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactImportantDateTypes.def()
    }
}

impl Related<super::contact_vault_user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactVaultUser.def()
    }
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::files::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Files.def()
    }
}

impl Related<super::groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Groups.def()
    }
}

impl Related<super::journals::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Journals.def()
    }
}

impl Related<super::labels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Labels.def()
    }
}

impl Related<super::life_event_categories::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEventCategories.def()
    }
}

impl Related<super::life_metrics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeMetrics.def()
    }
}

impl Related<super::loans::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Loans.def()
    }
}

impl Related<super::mood_tracking_parameters::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MoodTrackingParameters.def()
    }
}

impl Related<super::notes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Notes.def()
    }
}

impl Related<super::tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tags.def()
    }
}

impl Related<super::templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Templates.def()
    }
}

impl Related<super::timeline_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TimelineEvents.def()
    }
}

impl Related<super::user_vault::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserVault.def()
    }
}

impl Related<super::vault_quick_facts_templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::VaultQuickFactsTemplates.def()
    }
}

timestamps!();

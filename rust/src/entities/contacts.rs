use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contacts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub vault_id: Uuid,
    pub gender_id: Option<Id>,
    pub pronoun_id: Option<Id>,
    pub template_id: Option<Id>,
    pub company_id: Option<Id>,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub last_name: Option<String>,
    pub nickname: Option<String>,
    pub maiden_name: Option<String>,
    pub suffix: Option<String>,
    pub prefix: Option<String>,
    pub job_position: Option<String>,
    pub can_be_deleted: bool,
    pub listed: bool,
    #[sea_orm(column_type = "Text", nullable)]
    pub vcard: Option<String>,
    pub distant_uuid: Option<String>,
    pub distant_etag: Option<String>,
    pub distant_uri: Option<String>,
    pub last_updated_at: Option<Timestamp>,
    pub deleted_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    pub file_id: Option<Id>,
    pub religion_id: Option<Id>,
    pub show_quick_facts: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::calls::Entity")]
    Calls,
    #[sea_orm(
        belongs_to = "super::companies::Entity",
        from = "Column::CompanyId",
        to = "super::companies::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Companies,
    #[sea_orm(has_many = "super::contact_address::Entity")]
    ContactAddress,
    #[sea_orm(has_many = "super::contact_feed_items::Entity")]
    ContactFeedItems,
    #[sea_orm(has_many = "super::contact_group::Entity")]
    ContactGroup,
    #[sea_orm(has_many = "super::contact_important_dates::Entity")]
    ContactImportantDates,
    #[sea_orm(has_many = "super::contact_information::Entity")]
    ContactInformation,
    #[sea_orm(has_many = "super::contact_label::Entity")]
    ContactLabel,
    #[sea_orm(has_many = "super::contact_life_metric::Entity")]
    ContactLifeMetric,
    #[sea_orm(has_many = "super::contact_post::Entity")]
    ContactPost,
    #[sea_orm(has_many = "super::contact_reminders::Entity")]
    ContactReminders,
    #[sea_orm(has_many = "super::contact_tasks::Entity")]
    ContactTasks,
    #[sea_orm(has_many = "super::contact_vault_user::Entity")]
    ContactVaultUser,
    #[sea_orm(
        belongs_to = "super::files::Entity",
        from = "Column::FileId",
        to = "super::files::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Files,
    #[sea_orm(
        belongs_to = "super::genders::Entity",
        from = "Column::GenderId",
        to = "super::genders::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Genders,
    #[sea_orm(has_many = "super::gifts::Entity")]
    Gifts,
    #[sea_orm(has_many = "super::goals::Entity")]
    Goals,
    #[sea_orm(has_many = "super::life_event_participants::Entity")]
    LifeEventParticipants,
    #[sea_orm(has_many = "super::life_events::Entity")]
    LifeEvents,
    #[sea_orm(has_many = "super::mood_tracking_events::Entity")]
    MoodTrackingEvents,
    #[sea_orm(has_many = "super::notes::Entity")]
    Notes,
    #[sea_orm(has_many = "super::pets::Entity")]
    Pets,
    #[sea_orm(
        belongs_to = "super::pronouns::Entity",
        from = "Column::PronounId",
        to = "super::pronouns::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Pronouns,
    #[sea_orm(has_many = "super::quick_facts::Entity")]
    QuickFacts,
    #[sea_orm(
        belongs_to = "super::religions::Entity",
        from = "Column::ReligionId",
        to = "super::religions::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Religions,
    #[sea_orm(
        belongs_to = "super::templates::Entity",
        from = "Column::TemplateId",
        to = "super::templates::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Templates,
    #[sea_orm(has_many = "super::timeline_event_participants::Entity")]
    TimelineEventParticipants,
    #[sea_orm(has_many = "super::user_vault::Entity")]
    UserVault,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::calls::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Calls.def()
    }
}

impl Related<super::companies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Companies.def()
    }
}

impl Related<super::contact_address::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactAddress.def()
    }
}

impl Related<super::contact_feed_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactFeedItems.def()
    }
}

impl Related<super::contact_group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactGroup.def()
    }
}

impl Related<super::contact_important_dates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactImportantDates.def()
    }
}

impl Related<super::contact_information::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactInformation.def()
    }
}

impl Related<super::contact_label::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactLabel.def()
    }
}

impl Related<super::contact_life_metric::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactLifeMetric.def()
    }
}

impl Related<super::contact_post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactPost.def()
    }
}

impl Related<super::contact_reminders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactReminders.def()
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

impl Related<super::files::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Files.def()
    }
}

impl Related<super::genders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Genders.def()
    }
}

impl Related<super::gifts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Gifts.def()
    }
}

impl Related<super::goals::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Goals.def()
    }
}

impl Related<super::life_event_participants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEventParticipants.def()
    }
}

impl Related<super::life_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEvents.def()
    }
}

impl Related<super::mood_tracking_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MoodTrackingEvents.def()
    }
}

impl Related<super::notes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Notes.def()
    }
}

impl Related<super::pets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pets.def()
    }
}

impl Related<super::pronouns::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pronouns.def()
    }
}

impl Related<super::quick_facts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::QuickFacts.def()
    }
}

impl Related<super::religions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Religions.def()
    }
}

impl Related<super::templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Templates.def()
    }
}

impl Related<super::timeline_event_participants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TimelineEventParticipants.def()
    }
}

impl Related<super::user_vault::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserVault.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

timestamps!();

impl Entity {
    /// Rows not soft-deleted (Eloquent's `SoftDeletes` global scope).
    pub fn live() -> Select<Entity> {
        Self::find().filter(Column::DeletedAt.is_null())
    }
}

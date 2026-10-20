use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::account_currency::Entity")]
    AccountCurrency,
    #[sea_orm(has_many = "super::address_types::Entity")]
    AddressTypes,
    #[sea_orm(has_many = "super::call_reason_types::Entity")]
    CallReasonTypes,
    #[sea_orm(has_many = "super::contact_information_types::Entity")]
    ContactInformationTypes,
    #[sea_orm(has_many = "super::emotions::Entity")]
    Emotions,
    #[sea_orm(has_many = "super::genders::Entity")]
    Genders,
    #[sea_orm(has_many = "super::gift_occasions::Entity")]
    GiftOccasions,
    #[sea_orm(has_many = "super::gift_states::Entity")]
    GiftStates,
    #[sea_orm(has_many = "super::group_types::Entity")]
    GroupTypes,
    #[sea_orm(has_many = "super::modules::Entity")]
    Modules,
    #[sea_orm(has_many = "super::pet_categories::Entity")]
    PetCategories,
    #[sea_orm(has_many = "super::post_templates::Entity")]
    PostTemplates,
    #[sea_orm(has_many = "super::pronouns::Entity")]
    Pronouns,
    #[sea_orm(has_many = "super::relationship_group_types::Entity")]
    RelationshipGroupTypes,
    #[sea_orm(has_many = "super::religions::Entity")]
    Religions,
    #[sea_orm(has_many = "super::sync_tokens::Entity")]
    SyncTokens,
    #[sea_orm(has_many = "super::templates::Entity")]
    Templates,
    #[sea_orm(has_many = "super::users::Entity")]
    Users,
    #[sea_orm(has_many = "super::vaults::Entity")]
    Vaults,
}

impl Related<super::account_currency::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccountCurrency.def()
    }
}

impl Related<super::address_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AddressTypes.def()
    }
}

impl Related<super::call_reason_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CallReasonTypes.def()
    }
}

impl Related<super::contact_information_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactInformationTypes.def()
    }
}

impl Related<super::emotions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Emotions.def()
    }
}

impl Related<super::genders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Genders.def()
    }
}

impl Related<super::gift_occasions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GiftOccasions.def()
    }
}

impl Related<super::gift_states::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GiftStates.def()
    }
}

impl Related<super::group_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GroupTypes.def()
    }
}

impl Related<super::modules::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Modules.def()
    }
}

impl Related<super::pet_categories::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PetCategories.def()
    }
}

impl Related<super::post_templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostTemplates.def()
    }
}

impl Related<super::pronouns::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pronouns.def()
    }
}

impl Related<super::relationship_group_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RelationshipGroupTypes.def()
    }
}

impl Related<super::religions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Religions.def()
    }
}

impl Related<super::sync_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncTokens.def()
    }
}

impl Related<super::templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Templates.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

timestamps!();

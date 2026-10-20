use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "addresses")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub vault_id: Uuid,
    pub address_type_id: Option<Id>,
    pub line_1: Option<String>,
    pub line_2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    #[sea_orm(column_type = "Double", nullable)]
    pub latitude: Option<f64>,
    #[sea_orm(column_type = "Double", nullable)]
    pub longitude: Option<f64>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::address_types::Entity",
        from = "Column::AddressTypeId",
        to = "super::address_types::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    AddressTypes,
    #[sea_orm(has_many = "super::contact_address::Entity")]
    ContactAddress,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::address_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AddressTypes.def()
    }
}

impl Related<super::contact_address::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactAddress.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

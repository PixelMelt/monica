use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_address")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub contact_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub address_id: Id,
    pub is_past_address: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::addresses::Entity",
        from = "Column::AddressId",
        to = "super::addresses::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Addresses,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
}

impl Related<super::addresses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Addresses.def()
    }
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

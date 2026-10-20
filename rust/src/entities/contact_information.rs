use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_information")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub type_id: Id,
    pub data: String,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    pub kind: Option<String>,
    pub pref: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contact_information_types::Entity",
        from = "Column::TypeId",
        to = "super::contact_information_types::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ContactInformationTypes,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
}

impl Related<super::contact_information_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactInformationTypes.def()
    }
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

timestamps!();

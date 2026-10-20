use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "pets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub uuid: Option<Uuid>,
    pub contact_id: Uuid,
    pub pet_category_id: Id,
    pub name: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
    #[sea_orm(
        belongs_to = "super::pet_categories::Entity",
        from = "Column::PetCategoryId",
        to = "super::pet_categories::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    PetCategories,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::pet_categories::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PetCategories.def()
    }
}

timestamps!();

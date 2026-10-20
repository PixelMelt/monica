use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "goals")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub name: String,
    pub active: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
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
    #[sea_orm(has_many = "super::streaks::Entity")]
    Streaks,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::streaks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Streaks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

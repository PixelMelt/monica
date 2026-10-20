use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "slice_of_lives")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub journal_id: Id,
    pub file_cover_image_id: Option<Id>,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::files::Entity",
        from = "Column::FileCoverImageId",
        to = "super::files::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Files,
    #[sea_orm(
        belongs_to = "super::journals::Entity",
        from = "Column::JournalId",
        to = "super::journals::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Journals,
    #[sea_orm(has_many = "super::posts::Entity")]
    Posts,
}

impl Related<super::files::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Files.def()
    }
}

impl Related<super::journals::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Journals.def()
    }
}

impl Related<super::posts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Posts.def()
    }
}

timestamps!();

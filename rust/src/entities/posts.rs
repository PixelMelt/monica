use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub journal_id: Id,
    pub published: bool,
    pub title: Option<String>,
    pub view_count: i32,
    pub written_at: DateTime,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub slice_of_life_id: Option<Id>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contact_post::Entity")]
    ContactPost,
    #[sea_orm(
        belongs_to = "super::journals::Entity",
        from = "Column::JournalId",
        to = "super::journals::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Journals,
    #[sea_orm(has_many = "super::post_metrics::Entity")]
    PostMetrics,
    #[sea_orm(has_many = "super::post_sections::Entity")]
    PostSections,
    #[sea_orm(has_many = "super::post_tag::Entity")]
    PostTag,
    #[sea_orm(
        belongs_to = "super::slice_of_lives::Entity",
        from = "Column::SliceOfLifeId",
        to = "super::slice_of_lives::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    SliceOfLives,
}

impl Related<super::contact_post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactPost.def()
    }
}

impl Related<super::journals::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Journals.def()
    }
}

impl Related<super::post_metrics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostMetrics.def()
    }
}

impl Related<super::post_sections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostSections.def()
    }
}

impl Related<super::post_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostTag.def()
    }
}

impl Related<super::slice_of_lives::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SliceOfLives.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "post_sections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub post_id: Id,
    pub position: i32,
    pub label: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub content: Option<String>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::posts::Entity",
        from = "Column::PostId",
        to = "super::posts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Posts,
}

impl Related<super::posts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Posts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

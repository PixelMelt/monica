use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "post_tag")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag_id: Id,
    #[sea_orm(primary_key, auto_increment = false)]
    pub post_id: Id,
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
    #[sea_orm(
        belongs_to = "super::tags::Entity",
        from = "Column::TagId",
        to = "super::tags::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Tags,
}

impl Related<super::posts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Posts.def()
    }
}

impl Related<super::tags::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tags.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

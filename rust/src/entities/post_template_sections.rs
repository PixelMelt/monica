use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "post_template_sections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub post_template_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub position: i32,
    pub can_be_deleted: bool,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::post_templates::Entity",
        from = "Column::PostTemplateId",
        to = "super::post_templates::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    PostTemplates,
}

impl Related<super::post_templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PostTemplates.def()
    }
}

timestamps!();

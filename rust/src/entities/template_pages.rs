use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "template_pages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub template_id: Id,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub slug: String,
    pub position: Option<i32>,
    pub r#type: Option<String>,
    pub can_be_deleted: bool,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::module_template_page::Entity")]
    ModuleTemplatePage,
    #[sea_orm(
        belongs_to = "super::templates::Entity",
        from = "Column::TemplateId",
        to = "super::templates::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Templates,
}

impl Related<super::module_template_page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModuleTemplatePage.def()
    }
}

impl Related<super::templates::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Templates.def()
    }
}

timestamps!();

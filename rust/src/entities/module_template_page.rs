use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "module_template_page")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub template_page_id: Id,
    #[sea_orm(primary_key, auto_increment = false)]
    pub module_id: Id,
    pub position: Option<i32>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::modules::Entity",
        from = "Column::ModuleId",
        to = "super::modules::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Modules,
    #[sea_orm(
        belongs_to = "super::template_pages::Entity",
        from = "Column::TemplatePageId",
        to = "super::template_pages::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    TemplatePages,
}

impl Related<super::modules::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Modules.def()
    }
}

impl Related<super::template_pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TemplatePages.def()
    }
}

timestamps!();

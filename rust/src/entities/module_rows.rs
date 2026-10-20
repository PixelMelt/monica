use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "module_rows")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub module_id: Id,
    pub position: Option<i32>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::module_row_fields::Entity")]
    ModuleRowFields,
    #[sea_orm(
        belongs_to = "super::modules::Entity",
        from = "Column::ModuleId",
        to = "super::modules::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Modules,
}

impl Related<super::module_row_fields::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModuleRowFields.def()
    }
}

impl Related<super::modules::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Modules.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

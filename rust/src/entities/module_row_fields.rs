use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "module_row_fields")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub module_row_id: Id,
    pub label: String,
    pub module_field_type: String,
    pub required: bool,
    pub position: Option<i32>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::module_rows::Entity",
        from = "Column::ModuleRowId",
        to = "super::module_rows::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ModuleRows,
}

impl Related<super::module_rows::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModuleRows.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

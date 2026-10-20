use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "modules")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub account_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub r#type: Option<String>,
    pub reserved_to_contact_information: bool,
    pub can_be_deleted: bool,
    pub pagination: Option<i32>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Accounts,
    #[sea_orm(has_many = "super::module_rows::Entity")]
    ModuleRows,
    #[sea_orm(has_many = "super::module_template_page::Entity")]
    ModuleTemplatePage,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Accounts.def()
    }
}

impl Related<super::module_rows::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModuleRows.def()
    }
}

impl Related<super::module_template_page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModuleTemplatePage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

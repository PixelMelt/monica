use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "group_type_roles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub group_type_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub position: Option<i32>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contact_group::Entity")]
    ContactGroup,
    #[sea_orm(
        belongs_to = "super::group_types::Entity",
        from = "Column::GroupTypeId",
        to = "super::group_types::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    GroupTypes,
}

impl Related<super::contact_group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactGroup.def()
    }
}

impl Related<super::group_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GroupTypes.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use crate::db::id::Id;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "relationship_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub relationship_group_type_id: Id,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub name_reverse_relationship: Option<String>,
    pub name_reverse_relationship_translation_key: Option<String>,
    pub r#type: Option<String>,
    pub can_be_deleted: bool,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::relationship_group_types::Entity",
        from = "Column::RelationshipGroupTypeId",
        to = "super::relationship_group_types::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    RelationshipGroupTypes,
    #[sea_orm(has_many = "super::relationships::Entity")]
    Relationships,
}

impl Related<super::relationship_group_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RelationshipGroupTypes.def()
    }
}

impl Related<super::relationships::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Relationships.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

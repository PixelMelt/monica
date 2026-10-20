use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "relationships")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub relationship_type_id: Id,
    pub contact_id: Uuid,
    pub related_contact_id: Uuid,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts2,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::RelatedContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts1,
    #[sea_orm(
        belongs_to = "super::relationship_types::Entity",
        from = "Column::RelationshipTypeId",
        to = "super::relationship_types::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    RelationshipTypes,
}

impl Related<super::relationship_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RelationshipTypes.def()
    }
}

timestamps!();

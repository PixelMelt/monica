use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "call_reasons")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub call_reason_type_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::call_reason_types::Entity",
        from = "Column::CallReasonTypeId",
        to = "super::call_reason_types::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    CallReasonTypes,
    #[sea_orm(has_many = "super::calls::Entity")]
    Calls,
}

impl Related<super::call_reason_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CallReasonTypes.def()
    }
}

impl Related<super::calls::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Calls.def()
    }
}

timestamps!();

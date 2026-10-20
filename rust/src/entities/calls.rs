use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "calls")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub call_reason_id: Option<Id>,
    pub author_id: Option<Uuid>,
    pub emotion_id: Option<Id>,
    pub author_name: String,
    pub called_at: Timestamp,
    pub duration: Option<i32>,
    pub r#type: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub answered: bool,
    pub who_initiated: String,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::call_reasons::Entity",
        from = "Column::CallReasonId",
        to = "super::call_reasons::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    CallReasons,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
    #[sea_orm(
        belongs_to = "super::emotions::Entity",
        from = "Column::EmotionId",
        to = "super::emotions::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Emotions,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::AuthorId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Users,
}

impl Related<super::call_reasons::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CallReasons.def()
    }
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::emotions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Emotions.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

timestamps!();

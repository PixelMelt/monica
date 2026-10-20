use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub author_id: Option<Uuid>,
    pub author_name: String,
    pub label: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub completed: bool,
    pub completed_at: Option<Timestamp>,
    pub due_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
    pub uuid: Option<Uuid>,
    #[sea_orm(column_type = "Text", nullable)]
    pub vcalendar: Option<String>,
    pub distant_uuid: Option<String>,
    pub distant_etag: Option<String>,
    pub distant_uri: Option<String>,
    pub deleted_at: Option<Timestamp>,
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
    Contacts,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::AuthorId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Users,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

timestamps!();

impl Entity {
    /// Rows not soft-deleted (Eloquent's `SoftDeletes` global scope).
    pub fn live() -> Select<Entity> {
        Self::find().filter(Column::DeletedAt.is_null())
    }
}

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contact_important_dates")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub contact_id: Uuid,
    pub contact_important_date_type_id: Option<Id>,
    pub label: String,
    pub day: Option<i32>,
    pub month: Option<i32>,
    pub year: Option<i32>,
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
        belongs_to = "super::contact_important_date_types::Entity",
        from = "Column::ContactImportantDateTypeId",
        to = "super::contact_important_date_types::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    ContactImportantDateTypes,
    #[sea_orm(
        belongs_to = "super::contacts::Entity",
        from = "Column::ContactId",
        to = "super::contacts::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Contacts,
}

impl Related<super::contact_important_date_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactImportantDateTypes.def()
    }
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

timestamps!();

impl Entity {
    /// Rows not soft-deleted (Eloquent's `SoftDeletes` global scope).
    pub fn live() -> Select<Entity> {
        Self::find().filter(Column::DeletedAt.is_null())
    }
}

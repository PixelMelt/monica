use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "files")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub vault_id: Uuid,
    pub fileable_type: Option<String>,
    pub fileable_id: Option<Id>,
    pub ufileable_id: Option<Uuid>,
    pub uuid: String,
    pub original_url: Option<String>,
    pub cdn_url: Option<String>,
    pub mime_type: String,
    pub name: String,
    pub r#type: String,
    pub size: i32,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contacts::Entity")]
    Contacts,
    #[sea_orm(has_many = "super::slice_of_lives::Entity")]
    SliceOfLives,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::contacts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

impl Related<super::slice_of_lives::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SliceOfLives.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

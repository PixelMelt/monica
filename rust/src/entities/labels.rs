use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "labels")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub vault_id: Uuid,
    pub name: String,
    pub slug: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub bg_color: String,
    pub text_color: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contact_label::Entity")]
    ContactLabel,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::contact_label::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactLabel.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

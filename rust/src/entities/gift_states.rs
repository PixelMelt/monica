use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "gift_states")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub account_id: Uuid,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub position: Option<i32>,
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
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Accounts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

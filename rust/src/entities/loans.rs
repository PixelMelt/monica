use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "loans")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub vault_id: Uuid,
    pub r#type: String,
    pub name: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub amount_lent: Option<i32>,
    pub currency_id: Option<Id>,
    pub loaned_at: Option<DateTime>,
    pub settled: bool,
    pub settled_at: Option<DateTime>,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contact_gift::Entity")]
    ContactGift,
    #[sea_orm(has_many = "super::contact_loan::Entity")]
    ContactLoan,
    #[sea_orm(
        belongs_to = "super::currencies::Entity",
        from = "Column::CurrencyId",
        to = "super::currencies::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Currencies,
    #[sea_orm(
        belongs_to = "super::vaults::Entity",
        from = "Column::VaultId",
        to = "super::vaults::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Vaults,
}

impl Related<super::contact_gift::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactGift.def()
    }
}

impl Related<super::contact_loan::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContactLoan.def()
    }
}

impl Related<super::currencies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Currencies.def()
    }
}

impl Related<super::vaults::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vaults.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

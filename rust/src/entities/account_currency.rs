use crate::db::id::Id;
use crate::db::uuid::Uuid;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "account_currency")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub currency_id: Id,
    pub account_id: Uuid,
    pub active: bool,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
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
    #[sea_orm(
        belongs_to = "super::currencies::Entity",
        from = "Column::CurrencyId",
        to = "super::currencies::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Currencies,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Accounts.def()
    }
}

impl Related<super::currencies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Currencies.def()
    }
}

timestamps!();

use crate::db::id::Id;
use sea_orm::entity::prelude::*;

use crate::db::timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "currencies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Id,
    pub code: String,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::account_currency::Entity")]
    AccountCurrency,
    #[sea_orm(has_many = "super::gifts::Entity")]
    Gifts,
    #[sea_orm(has_many = "super::life_events::Entity")]
    LifeEvents,
    #[sea_orm(has_many = "super::loans::Entity")]
    Loans,
}

impl Related<super::account_currency::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccountCurrency.def()
    }
}

impl Related<super::gifts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Gifts.def()
    }
}

impl Related<super::life_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LifeEvents.def()
    }
}

impl Related<super::loans::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Loans.def()
    }
}

timestamps!();

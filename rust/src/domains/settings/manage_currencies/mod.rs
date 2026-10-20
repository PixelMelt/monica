//! `app/Domains/Settings/ManageCurrencies/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{account_currency, currencies};
use crate::error::ApiResult;
use crate::validate::Validator;

/// `DisableAllCurrencies`.
pub struct DisableAllCurrencies {
    pub account_id: Uuid,
    pub author_id: Uuid,
}

impl DisableAllCurrencies {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        account_currency::Entity::update_many()
            .col_expr(account_currency::Column::Active, Expr::value(false))
            .filter(account_currency::Column::AccountId.eq(self.account_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `EnableAllCurrencies`.
pub struct EnableAllCurrencies {
    pub account_id: Uuid,
    pub author_id: Uuid,
}

impl EnableAllCurrencies {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        account_currency::Entity::update_many()
            .col_expr(account_currency::Column::Active, Expr::value(true))
            .filter(account_currency::Column::AccountId.eq(self.account_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `ToggleCurrency`.
pub struct ToggleCurrency {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub currency_id: Id,
}

impl ToggleCurrency {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        if currencies::Entity::find_by_id(self.currency_id).one(db).await?.is_none() {
            v.fail("currency_id", "exists", &[]);
        }
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        account_currency::Entity::update_many()
            .col_expr(account_currency::Column::Active, Expr::col(account_currency::Column::Active).not())
            .filter(account_currency::Column::AccountId.eq(self.account_id))
            .filter(account_currency::Column::CurrencyId.eq(self.currency_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

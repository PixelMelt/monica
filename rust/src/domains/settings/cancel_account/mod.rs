//! `app/Domains/Settings/CancelAccount/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{accounts, files, vaults};
use crate::error::{ApiError, ApiResult};

/// `CancelAccount`.
pub struct CancelAccount {
    pub account_id: Uuid,
    pub author_id: Uuid,
}

impl CancelAccount {
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

        let account = accounts::Entity::find_by_id(self.account_id).one(db).await?.ok_or_else(ApiError::not_found)?;
        Self::destroy_all_files(db, &account).await?;

        accounts::Entity::delete_by_id(account.id).exec(db).await?;
        Ok(())
    }

    async fn destroy_all_files<C: ConnectionTrait>(db: &C, account: &accounts::Model) -> ApiResult<()> {
        let vault_ids: Vec<Uuid> = vaults::Entity::find()
            .filter(vaults::Column::AccountId.eq(account.id))
            .all(db)
            .await?
            .into_iter()
            .map(|v| v.id)
            .collect();

        for file in files::Entity::find().filter(files::Column::VaultId.is_in(vault_ids)).all(db).await? {
            files::Entity::delete_by_id(file.id).exec(db).await?;
        }
        Ok(())
    }
}

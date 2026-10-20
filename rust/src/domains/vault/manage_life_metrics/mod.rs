//! `app/Domains/Vault/ManageLifeMetrics/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contact_life_metric, contacts, life_metrics, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateLifeMetric`.
pub struct CreateLifeMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub label: String,
}

impl CreateLifeMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_metrics::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        Ok(life_metrics::ActiveModel { vault_id: Set(self.vault_id), label: Set(self.label), ..Default::default() }
            .insert(db)
            .await?)
    }
}

/// `DestroyLifeMetric`.
pub struct DestroyLifeMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_metric_id: Id,
}

impl DestroyLifeMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let life_metric = find(db, checked.vault().id, self.life_metric_id).await?;

        life_metrics::Entity::delete_by_id(life_metric.id).exec(db).await?;
        Ok(())
    }
}

/// `IncrementLifeMetric`.
pub struct IncrementLifeMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_metric_id: Id,
}

impl IncrementLifeMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_metrics::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let life_metric = find(db, checked.vault().id, self.life_metric_id).await?;

        let contact = get_contact_in_vault(db, &checked.author, checked.vault())
            .await?
            .ok_or_else(|| ApiError::internal("the author has no contact in the vault"))?;
        contact_life_metric::ActiveModel {
            contact_id: Set(contact.id),
            life_metric_id: Set(life_metric.id),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(life_metric)
    }
}

/// `UpdateLifeMetric`.
pub struct UpdateLifeMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_metric_id: Id,
    pub label: String,
}

impl UpdateLifeMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_metrics::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let life_metric = find(db, checked.vault().id, self.life_metric_id).await?;

        let mut life_metric: life_metrics::ActiveModel = life_metric.into();
        life_metric.label = Set(self.label);
        Ok(life_metric.update(db).await?)
    }
}

/// `$this->vault->lifeMetrics()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, vault_id: Uuid, life_metric_id: Id) -> ApiResult<life_metrics::Model> {
    life_metrics::Entity::find_by_id(life_metric_id)
        .filter(life_metrics::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `User::getContactInVault($vault)`: the user's contact in the vault, `None`
/// when the user isn't a member or the contact is (soft-)deleted.
async fn get_contact_in_vault<C: ConnectionTrait>(
    db: &C,
    user: &users::Model,
    vault: &vaults::Model,
) -> ApiResult<Option<contacts::Model>> {
    let entry = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user.id))
        .filter(user_vault::Column::VaultId.eq(vault.id))
        .one(db)
        .await?;

    let Some(entry) = entry else {
        return Ok(None);
    };

    Ok(contacts::Entity::live().filter(contacts::Column::Id.eq(entry.contact_id)).one(db).await?)
}

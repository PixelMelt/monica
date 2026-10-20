//! `app/Domains/Contact/ManagePronouns/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contacts, pronouns};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `RemovePronoun`.
pub struct RemovePronoun {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl RemovePronoun {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.pronoun_id = Set(None);
        let contact = contact.update(db).await?;

        let mut contact: contacts::ActiveModel = contact.into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;
        Ok(())
    }
}

/// `SetPronoun`.
pub struct SetPronoun {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub pronoun_id: Id,
}

impl SetPronoun {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        if pronouns::Entity::find_by_id(self.pronoun_id).one(db).await?.is_none() {
            v.fail("pronoun_id", "exists", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let pronoun = pronouns::Entity::find_by_id(self.pronoun_id)
            .filter(pronouns::Column::AccountId.eq(self.account_id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.pronoun_id = Set(Some(pronoun.id));
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;
        Ok(())
    }
}

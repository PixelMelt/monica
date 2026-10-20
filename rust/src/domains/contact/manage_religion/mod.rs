//! `app/Domains/Contact/ManageReligion/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contact_feed_items, contacts, religions};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `UpdateReligion`.
pub struct UpdateReligion {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub religion_id: Option<Id>,
}

impl UpdateReligion {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        if let Some(id) = self.religion_id
            && religions::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("religion_id", "exists", &[]);
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

        // F10: the PHP runs `findOrFail(null)` when the religion is cleared,
        // which answered 404; only a given religion is looked up.
        let religion = match self.religion_id {
            Some(id) => Some(
                religions::Entity::find_by_id(id)
                    .filter(religions::Column::AccountId.eq(self.account_id))
                    .one(db)
                    .await?
                    .ok_or_else(ApiError::not_found)?,
            ),
            None => None,
        };

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.religion_id = Set(religion.map(|r| r.id));
        let contact = contact.update(db).await?;

        let mut contact: contacts::ActiveModel = contact.into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        let contact = contact.update(db).await?;

        contact_feed_items::ActiveModel {
            author_id: Set(Some(checked.author.id)),
            contact_id: Set(contact.id),
            action: Set("religion_updated".into()),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(contact)
    }
}

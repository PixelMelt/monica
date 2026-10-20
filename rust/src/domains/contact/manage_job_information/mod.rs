//! `app/Domains/Contact/ManageJobInformation/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{companies, contact_feed_items, contacts, users};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `ResetJobInformation`.
pub struct ResetJobInformation {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl ResetJobInformation {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
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
        contact.company_id = Set(None);
        contact.job_position = Set(None);
        let contact = contact.update(db).await?;

        let contact = update_last_edited_date(db, contact).await?;
        create_feed_item(db, &checked.author, &contact).await?;

        Ok(contact)
    }
}

/// `UpdateJobInformation`.
pub struct UpdateJobInformation {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub company_id: Option<Id>,
    pub job_position: Option<String>,
}

impl UpdateJobInformation {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        v.max_len("job_position", self.job_position.as_deref(), 255);
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

        let company = match self.company_id {
            Some(id) => Some(
                companies::Entity::find_by_id(id)
                    .filter(companies::Column::VaultId.eq(checked.vault().id))
                    .one(db)
                    .await?
                    .ok_or_else(ApiError::not_found)?,
            ),
            None => None,
        };

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.company_id = Set(company.map(|c| c.id));
        contact.job_position = Set(value_or_null(self.job_position));
        let contact = contact.update(db).await?;

        let contact = update_last_edited_date(db, contact).await?;
        create_feed_item(db, &checked.author, &contact).await?;

        Ok(contact)
    }
}

/// `updateLastEditedDate`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: contacts::Model) -> ApiResult<contacts::Model> {
    let mut contact: contacts::ActiveModel = contact.into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    Ok(contact.update(db).await?)
}

/// The `job_information_updated` feed item.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact.id),
        action: Set("job_information_updated".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

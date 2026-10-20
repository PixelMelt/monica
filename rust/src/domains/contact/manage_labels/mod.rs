//! `app/Domains/Contact/ManageLabels/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contact_feed_items, contact_label, contacts, labels, users};
use crate::error::{ApiError, ApiResult};

/// `ContactFeedItem::ACTION_LABEL_ASSIGNED`.
const ACTION_LABEL_ASSIGNED: &str = "label_assigned";
/// `ContactFeedItem::ACTION_LABEL_REMOVED`.
const ACTION_LABEL_REMOVED: &str = "label_removed";

/// `AssignLabel`.
pub struct AssignLabel {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub label_id: Id,
}

impl AssignLabel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<labels::Model> {
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

        let label = find(db, checked.vault().id, self.label_id).await?;

        let attached = contact_label::Entity::find()
            .filter(contact_label::Column::ContactId.eq(checked.contact().id))
            .filter(contact_label::Column::LabelId.eq(label.id))
            .one(db)
            .await?;
        if attached.is_none() {
            contact_label::Entity::insert(contact_label::ActiveModel {
                label_id: Set(label.id),
                contact_id: Set(checked.contact().id),
                ..Default::default()
            })
            .exec_without_returning(db)
            .await?;
        }

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked.author, checked.contact(), &label, ACTION_LABEL_ASSIGNED).await?;

        Ok(label)
    }
}

/// `RemoveLabel`.
pub struct RemoveLabel {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub label_id: Id,
}

impl RemoveLabel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<labels::Model> {
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

        let label = find(db, checked.vault().id, self.label_id).await?;

        contact_label::Entity::delete_many()
            .filter(contact_label::Column::ContactId.eq(checked.contact().id))
            .filter(contact_label::Column::LabelId.eq(label.id))
            .exec(db)
            .await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked.author, checked.contact(), &label, ACTION_LABEL_REMOVED).await?;

        Ok(label)
    }
}

/// `createFeedItem()` of AssignLabel and RemoveLabel, which differ only by
/// the action.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    contact: &contacts::Model,
    label: &labels::Model,
    action: &str,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact.id),
        action: Set(action.into()),
        description: Set(Some(label.name.clone())),
        feedable_type: Set(Some("App\\Models\\Label".into())),
        feedable_id: Set(Some(label.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `$this->vault->labels()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, vault_id: Uuid, label_id: Id) -> ApiResult<labels::Model> {
    labels::Entity::find_by_id(label_id)
        .filter(labels::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

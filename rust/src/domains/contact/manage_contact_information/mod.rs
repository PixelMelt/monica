//! `app/Domains/Contact/ManageContactInformation/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check, value_or_null};
use crate::entities::{contact_feed_items, contact_information, contact_information_types, contacts};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::validate::Validator;

/// `CreateContactInformation`.
pub struct CreateContactInformation {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_information_type_id: Id,
    pub contact_information_kind: Option<String>,
    pub data: String,
}

impl CreateContactInformation {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_information::Model> {
        let mut v = Validator::new();
        type_exists(db, &mut v, self.contact_information_type_id).await?;
        data_rules(&mut v, &self.data);
        v.finish()?;
        let checked = check(db, &permissions(self.account_id, self.author_id, self.vault_id, self.contact_id)).await?;

        let r#type = find_type(db, self.account_id, self.contact_information_type_id).await?;

        let contact_information = contact_information::ActiveModel {
            contact_id: Set(checked.contact().id),
            type_id: Set(r#type.id),
            data: Set(self.data),
            kind: Set(value_or_null(self.contact_information_kind)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        update_last_edited_date(db, &checked).await?;
        let description = name(&contact_information, &r#type);
        create_feed_item(db, &checked, "contact_information_created", description, Some(&contact_information)).await?;

        Ok(contact_information)
    }
}

/// `DestroyContactInformation`.
pub struct DestroyContactInformation {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_information_id: Id,
}

impl DestroyContactInformation {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(db, &permissions(self.account_id, self.author_id, self.vault_id, self.contact_id)).await?;

        let contact_information = find_information(db, &checked, self.contact_information_id).await?;

        let r#type = find_type(db, self.account_id, contact_information.type_id).await?;

        contact_information::Entity::delete_by_id(contact_information.id).exec(db).await?;

        update_last_edited_date(db, &checked).await?;

        let description = name(&contact_information, &r#type);
        create_feed_item(db, &checked, "contact_information_destroyed", description, None).await
    }
}

/// `UpdateContactInformation`.
pub struct UpdateContactInformation {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_information_type_id: Id,
    pub contact_information_id: Id,
    pub contact_information_kind: Option<String>,
    pub data: String,
}

impl UpdateContactInformation {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_information::Model> {
        let mut v = Validator::new();
        type_exists(db, &mut v, self.contact_information_type_id).await?;
        data_rules(&mut v, &self.data);
        v.finish()?;
        let checked = check(db, &permissions(self.account_id, self.author_id, self.vault_id, self.contact_id)).await?;

        let r#type = find_type(db, self.account_id, self.contact_information_type_id).await?;

        let contact_information = find_information(db, &checked, self.contact_information_id).await?;

        let mut contact_information: contact_information::ActiveModel = contact_information.into();
        contact_information.data = Set(self.data);
        contact_information.type_id = Set(self.contact_information_type_id);
        contact_information.kind = Set(value_or_null(self.contact_information_kind));
        let contact_information = contact_information.update(db).await?;

        update_last_edited_date(db, &checked).await?;
        let description = name(&contact_information, &r#type);
        create_feed_item(db, &checked, "contact_information_updated", description, Some(&contact_information)).await?;

        Ok(contact_information)
    }
}

/// `permissions()`: vault editor, contact in the vault.
fn permissions(account_id: Uuid, author_id: Uuid, vault_id: Uuid, contact_id: Uuid) -> Check {
    Check {
        account_id,
        author_id,
        vault: Some((vault_id, Some(VaultPermission::Edit))),
        contact_id: Some(contact_id),
        ..Default::default()
    }
}

/// `exists:contact_information_types,id`.
async fn type_exists<C: ConnectionTrait>(db: &C, v: &mut Validator, id: Id) -> ApiResult<()> {
    if contact_information_types::Entity::find_by_id(id).one(db).await?.is_none() {
        v.fail("contact_information_type_id", "exists", &[]);
    }
    Ok(())
}

/// `data`: `required|string|max:255`.
fn data_rules(v: &mut Validator, data: &str) {
    if data.trim().is_empty() {
        v.fail("data", "required", &[]);
    }
    v.max_len("data", Some(data), 255);
}

/// `$this->account()->contactInformationTypes()->findOrFail($id)`.
async fn find_type<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    id: Id,
) -> ApiResult<contact_information_types::Model> {
    contact_information_types::Entity::find_by_id(id)
        .filter(contact_information_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->contact->contactInformations()->findOrFail($id)`.
async fn find_information<C: ConnectionTrait>(
    db: &C,
    checked: &Checked,
    id: Id,
) -> ApiResult<contact_information::Model> {
    contact_information::Entity::find_by_id(id)
        .filter(contact_information::Column::ContactId.eq(checked.contact().id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `ContactInformation::name`: the data for the types that can't be deleted
/// (phone, email), else the type's name (`ContactInformationType::name`).
fn name(contact_information: &contact_information::Model, r#type: &contact_information_types::Model) -> Option<String> {
    if !r#type.can_be_deleted {
        return Some(contact_information.data.clone());
    }
    match &r#type.name {
        Some(name) if !name.is_empty() => Some(name.clone()),
        _ => r#type.name_translation_key.as_deref().map(|key| i18n::t(&i18n::current(), key, &[])),
    }
}

async fn update_last_edited_date<C: ConnectionTrait>(db: &C, checked: &Checked) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = checked.contact().clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}

async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    checked: &Checked,
    action: &str,
    description: Option<String>,
    feedable: Option<&contact_information::Model>,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(checked.author.id)),
        contact_id: Set(checked.contact().id),
        action: Set(action.into()),
        description: Set(description),
        feedable_type: Set(feedable.map(|_| "App\\Models\\ContactInformation".into())),
        feedable_id: Set(feedable.map(|f| f.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

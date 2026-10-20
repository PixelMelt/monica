//! `app/Domains/Contact/ManagePets/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check, value_or_null};
use crate::entities::{contact_feed_items, contacts, pet_categories, pets};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::validate::Validator;

/// `ContactFeedItem::ACTION_PET_CREATED`.
const ACTION_PET_CREATED: &str = "pet_created";
/// `ContactFeedItem::ACTION_PET_UPDATED`.
const ACTION_PET_UPDATED: &str = "pet_updated";
/// `ContactFeedItem::ACTION_PET_DESTROYED`.
const ACTION_PET_DESTROYED: &str = "pet_destroyed";

/// `CreatePet`.
pub struct CreatePet {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub pet_category_id: Id,
    pub name: Option<String>,
}

impl CreatePet {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<pets::Model> {
        let mut v = Validator::new();
        pet_category_exists(db, &mut v, self.pet_category_id).await?;
        v.max_len("name", self.name.as_deref(), 255);
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

        let pet_category = find_pet_category(db, checked.author.account_id, self.pet_category_id).await?;

        let pet = pets::ActiveModel {
            contact_id: Set(checked.contact().id),
            pet_category_id: Set(self.pet_category_id),
            name: Set(value_or_null(self.name)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked, &pet, &pet_category, ACTION_PET_CREATED).await?;

        Ok(pet)
    }
}

/// `DestroyPet`.
pub struct DestroyPet {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub pet_id: Id,
}

impl DestroyPet {
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

        let pet = find(db, checked.contact().id, self.pet_id).await?;

        pets::Entity::delete_by_id(pet.id).exec(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Self::create_feed_item(db, &checked, &pet).await?;

        Ok(())
    }

    async fn create_feed_item<C: ConnectionTrait>(db: &C, checked: &Checked, pet: &pets::Model) -> ApiResult<()> {
        let pet_category = pet_categories::Entity::find_by_id(pet.pet_category_id)
            .one(db)
            .await?
            .ok_or_else(|| ApiError::internal("the pet has no category"))?;
        contact_feed_items::ActiveModel {
            author_id: Set(Some(checked.author.id)),
            contact_id: Set(checked.contact().id),
            action: Set(ACTION_PET_DESTROYED.into()),
            description: Set(name(&pet_category)),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }
}

/// `UpdatePet`.
pub struct UpdatePet {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub pet_id: Id,
    pub pet_category_id: Id,
    pub name: Option<String>,
}

impl UpdatePet {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<pets::Model> {
        let mut v = Validator::new();
        pet_category_exists(db, &mut v, self.pet_category_id).await?;
        v.max_len("name", self.name.as_deref(), 255);
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

        let pet_category = find_pet_category(db, checked.author.account_id, self.pet_category_id).await?;

        // F7: the PHP also filtered the pet by the new category, so changing
        // the category was a 404.
        let pet = find(db, checked.contact().id, self.pet_id).await?;

        let mut pet: pets::ActiveModel = pet.into();
        pet.pet_category_id = Set(self.pet_category_id);
        pet.name = Set(value_or_null(self.name));
        let pet = pet.update(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked, &pet, &pet_category, ACTION_PET_UPDATED).await?;

        Ok(pet)
    }
}

/// `createFeedItem()` of CreatePet and UpdatePet, which differ only by the
/// action. `pet_category` is `$this->pet->petCategory`.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    checked: &Checked,
    pet: &pets::Model,
    pet_category: &pet_categories::Model,
    action: &str,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(checked.author.id)),
        contact_id: Set(checked.contact().id),
        action: Set(action.into()),
        description: Set(name(pet_category)),
        feedable_type: Set(Some("App\\Models\\Pet".into())),
        feedable_id: Set(Some(pet.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `'pet_category_id' => 'required|integer|exists:pet_categories,id'`.
async fn pet_category_exists<C: ConnectionTrait>(db: &C, v: &mut Validator, pet_category_id: Id) -> ApiResult<()> {
    if pet_categories::Entity::find_by_id(pet_category_id).one(db).await?.is_none() {
        v.fail("pet_category_id", "exists", &[]);
    }
    Ok(())
}

/// `$this->account()->petCategories()->findOrFail($id)`.
async fn find_pet_category<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    pet_category_id: Id,
) -> ApiResult<pet_categories::Model> {
    pet_categories::Entity::find_by_id(pet_category_id)
        .filter(pet_categories::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->contact->pets()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, contact_id: Uuid, pet_id: Id) -> ApiResult<pets::Model> {
    pets::Entity::find_by_id(pet_id)
        .filter(pets::Column::ContactId.eq(contact_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `PetCategory::name()`: the name, else `__($name_translation_key)`.
fn name(pet_category: &pet_categories::Model) -> Option<String> {
    match pet_category.name.as_deref() {
        Some(name) if !name.is_empty() && name != "0" => Some(name.to_owned()),
        _ => pet_category.name_translation_key.as_deref().map(|key| i18n::t(&i18n::current(), key, &[])),
    }
}

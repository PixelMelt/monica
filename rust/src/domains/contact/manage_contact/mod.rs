//! `app/Domains/Contact/ManageContact/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, user_permission_in_vault, value_or_null};
use crate::domains::vault::manage_companies::CreateCompany;
use crate::entities::{
    companies, contact_feed_items, contact_vault_user, contacts, files, genders, pronouns, templates, users, vaults,
};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CopyContactToAnotherVault`.
pub struct CopyContactToAnotherVault {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub other_vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl CopyContactToAnotherVault {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        if vaults::Entity::find_by_id(self.other_vault_id).one(db).await?.is_none() {
            v.fail("other_vault_id", "exists", &[]);
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
        let new_vault = new_vault(db, self.account_id, &checked.author, self.other_vault_id).await?;

        let mut new_contact: contacts::ActiveModel = checked.contact().clone().into();
        new_contact.id = Set(Uuid::new());
        new_contact.vault_id = Set(new_vault.id);
        new_contact.created_at = NotSet;
        new_contact.updated_at = NotSet;
        let new_contact = new_contact.insert(db).await?;

        update_last_edited_date(db, checked.contact().clone()).await?;

        Ok(new_contact)
    }
}

/// `CreateContact`.
pub struct CreateContact {
    pub account_id: Uuid,
    pub id: Option<Uuid>,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub nickname: Option<String>,
    pub maiden_name: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub gender_id: Option<Id>,
    pub pronoun_id: Option<Id>,
    pub template_id: Option<Id>,
    pub listed: bool,
}

impl CreateContact {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        for (field, value) in [
            ("first_name", &self.first_name),
            ("last_name", &self.last_name),
            ("middle_name", &self.middle_name),
            ("nickname", &self.nickname),
            ("maiden_name", &self.maiden_name),
            ("prefix", &self.prefix),
            ("suffix", &self.suffix),
        ] {
            v.max_len(field, value.as_deref(), 255);
        }
        exists_gender_and_pronoun(db, &mut v, self.gender_id, self.pronoun_id).await?;
        if let Some(id) = self.template_id
            && templates::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("template_id", "exists", &[]);
        }
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

        if let Some(id) = self.gender_id {
            find_gender(db, self.account_id, id).await?;
        }
        if let Some(id) = self.pronoun_id {
            find_pronoun(db, self.account_id, id).await?;
        }
        if let Some(id) = self.template_id {
            find_template(db, self.account_id, id).await?;
        }

        // template - if no template is provided, we should use the default
        // template that is in the vault - if it exists.
        let template_id = self.template_id.or(checked.vault().default_template_id);

        let contact = contacts::ActiveModel {
            id: Set(self.id.unwrap_or_default()),
            vault_id: Set(self.vault_id),
            first_name: Set(value_or_null(self.first_name)),
            last_name: Set(value_or_null(self.last_name)),
            middle_name: Set(value_or_null(self.middle_name)),
            nickname: Set(value_or_null(self.nickname)),
            maiden_name: Set(value_or_null(self.maiden_name)),
            gender_id: Set(self.gender_id),
            pronoun_id: Set(self.pronoun_id),
            suffix: Set(value_or_null(self.suffix)),
            prefix: Set(value_or_null(self.prefix)),
            template_id: Set(template_id),
            last_updated_at: Set(Some(crate::db::now())),
            listed: Set(self.listed),
            ..Default::default()
        }
        .insert(db)
        .await?;

        let contact = update_last_edited_date(db, contact).await?;
        create_feed_item(db, &checked.author, &contact, "contact_created").await?;

        Ok(contact)
    }
}

/// `DestroyContact`.
pub struct DestroyContact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl DestroyContact {
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
        let contact = checked.contact().clone();

        if !contact.can_be_deleted {
            return Err(ApiError::conflict("The contact cannot be deleted."));
        }

        destroy_files(db, &contact).await?;

        let mut contact: contacts::ActiveModel = contact.into();
        contact.deleted_at = Set(Some(crate::db::now()));
        contact.update(db).await?;
        Ok(())
    }
}

async fn destroy_files<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let files = files::Entity::find()
        .filter(files::Column::FileableType.eq("App\\Models\\Contact"))
        .filter(files::Column::UfileableId.eq(contact.id))
        .all(db)
        .await?;
    for file in files {
        files::Entity::delete_by_id(file.id).exec(db).await?;
    }
    Ok(())
}

/// `MoveContactToAnotherVault`.
pub struct MoveContactToAnotherVault {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub other_vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl MoveContactToAnotherVault {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        if vaults::Entity::find_by_id(self.other_vault_id).one(db).await?.is_none() {
            v.fail("other_vault_id", "exists", &[]);
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
        let new_vault = new_vault(db, self.account_id, &checked.author, self.other_vault_id).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.vault_id = Set(new_vault.id);
        let contact = contact.update(db).await?;

        let contact = move_company_information(db, &checked.author, &new_vault, contact).await?;
        let contact = update_last_edited_date(db, contact).await?;

        Ok(contact)
    }
}

/// If the contact belongs to a company, we should move the company
/// information to the new vault as well.
/// If the company only has this contact, we should move the company.
/// However, if the company has other contacts, we should copy the company
/// and move the contact to the new company.
async fn move_company_information<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    new_vault: &vaults::Model,
    contact: contacts::Model,
) -> ApiResult<contacts::Model> {
    let company = match contact.company_id {
        Some(id) => companies::Entity::find_by_id(id).one(db).await?,
        None => None,
    };
    let Some(company) = company else { return Ok(contact) };

    if contacts::Entity::live().filter(contacts::Column::CompanyId.eq(company.id)).count(db).await? == 1 {
        let mut company: companies::ActiveModel = company.into();
        company.vault_id = Set(new_vault.id);
        company.update(db).await?;
        Ok(contact)
    } else {
        let new_company = CreateCompany {
            account_id: author.account_id,
            vault_id: new_vault.id,
            author_id: author.id,
            name: company.name,
            r#type: company.r#type,
        }
        .execute(db)
        .await?;

        let mut contact: contacts::ActiveModel = contact.into();
        contact.company_id = Set(Some(new_company.id));
        Ok(contact.update(db).await?)
    }
}

/// `validate()` of the copy and move services: `$this->account()->vaults()
/// ->findOrFail($other_vault_id)`, where the author must be an editor.
async fn new_vault<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    author: &users::Model,
    other_vault_id: Uuid,
) -> ApiResult<vaults::Model> {
    let new_vault = vaults::Entity::find_by_id(other_vault_id)
        .filter(vaults::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    user_permission_in_vault(db, author.id, new_vault.id, VaultPermission::Edit).await?;
    Ok(new_vault)
}

/// `ToggleArchiveContact`.
pub struct ToggleArchiveContact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl ToggleArchiveContact {
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

        let listed = checked.contact().listed;
        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.listed = Set(!listed);
        let contact = contact.update(db).await?;

        let contact = update_last_edited_date(db, contact).await?;
        let action = if contact.listed { "unarchived" } else { "archived" };
        create_feed_item(db, &checked.author, &contact, action).await?;

        Ok(contact)
    }
}

/// `ToggleFavoriteContact`.
pub struct ToggleFavoriteContact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl ToggleFavoriteContact {
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

        let is_favorite = self.toggle(db).await?;
        let contact = update_last_edited_date(db, checked.contact().clone()).await?;
        let action = if is_favorite { "unfavorited" } else { "favorited" };
        create_feed_item(db, &checked.author, &contact, action).await?;

        Ok(contact)
    }

    /// Whether the contact was a favorite before the toggle.
    async fn toggle<C: ConnectionTrait>(&self, db: &C) -> ApiResult<bool> {
        let exists =
            contact_vault_user::Entity::find_by_id((self.contact_id, self.vault_id, self.author_id)).one(db).await?;

        if let Some(exists) = exists {
            contact_vault_user::Entity::update_many()
                .col_expr(contact_vault_user::Column::IsFavorite, Expr::value(!exists.is_favorite))
                .filter(contact_vault_user::Column::ContactId.eq(self.contact_id))
                .filter(contact_vault_user::Column::VaultId.eq(self.vault_id))
                .filter(contact_vault_user::Column::UserId.eq(self.author_id))
                .exec(db)
                .await?;
            Ok(exists.is_favorite)
        } else {
            contact_vault_user::ActiveModel {
                contact_id: Set(self.contact_id),
                vault_id: Set(self.vault_id),
                user_id: Set(self.author_id),
                is_favorite: Set(true),
                number_of_views: Set(1),
                created_at: Set(None),
                updated_at: Set(None),
            }
            .insert(db)
            .await?;
            // F6: the PHP sets `isFavorite = true` here, so a first favorite
            // wrote an `unfavorited` feed item.
            Ok(false)
        }
    }
}

/// `UpdateContact`.
pub struct UpdateContact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub first_name: String,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub nickname: Option<String>,
    pub maiden_name: Option<String>,
    pub gender_id: Option<Id>,
    pub pronoun_id: Option<Id>,
    pub suffix: Option<String>,
    pub prefix: Option<String>,
}

impl UpdateContact {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        if self.first_name.trim().is_empty() {
            v.fail("first_name", "required", &[]);
        }
        for (field, value) in [
            ("first_name", Some(&self.first_name)),
            ("last_name", self.last_name.as_ref()),
            ("middle_name", self.middle_name.as_ref()),
            ("nickname", self.nickname.as_ref()),
            ("maiden_name", self.maiden_name.as_ref()),
            ("suffix", self.suffix.as_ref()),
            ("prefix", self.prefix.as_ref()),
        ] {
            v.max_len(field, value.map(String::as_str), 255);
        }
        exists_gender_and_pronoun(db, &mut v, self.gender_id, self.pronoun_id).await?;
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

        let gender = match self.gender_id {
            Some(id) => Some(find_gender(db, self.account_id, id).await?),
            None => None,
        };
        let pronoun = match self.pronoun_id {
            Some(id) => Some(find_pronoun(db, self.account_id, id).await?),
            None => None,
        };

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.first_name = Set(Some(self.first_name));
        contact.last_name = Set(value_or_null(self.last_name));
        contact.middle_name = Set(value_or_null(self.middle_name));
        contact.maiden_name = Set(value_or_null(self.maiden_name));
        contact.nickname = Set(value_or_null(self.nickname));
        contact.suffix = Set(value_or_null(self.suffix));
        contact.prefix = Set(value_or_null(self.prefix));
        contact.gender_id = Set(gender.map(|g| g.id));
        contact.pronoun_id = Set(pronoun.map(|p| p.id));
        contact.last_updated_at = Set(Some(crate::db::now()));
        let contact = contact.update(db).await?;

        let contact = update_last_edited_date(db, contact).await?;
        create_feed_item(db, &checked.author, &contact, "information_updated").await?;

        Ok(contact)
    }
}

/// `UpdateContactSortOrder`.
pub struct UpdateContactSortOrder {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub sort_order: String,
}

impl UpdateContactSortOrder {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        if self.sort_order.trim().is_empty() {
            v.fail("sort_order", "required", &[]);
        }
        v.max_len("sort_order", Some(&self.sort_order), 255);
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::View))),
                ..Default::default()
            },
        )
        .await?;

        let mut author: users::ActiveModel = checked.author.into();
        author.contact_sort_order = Set(self.sort_order);
        author.update(db).await?;
        Ok(())
    }
}

/// `UpdateContactTemplate`.
pub struct UpdateContactTemplate {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub template_id: Id,
}

impl UpdateContactTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        if templates::Entity::find_by_id(self.template_id).one(db).await?.is_none() {
            v.fail("template_id", "exists", &[]);
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

        find_template(db, self.account_id, self.template_id).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.template_id = Set(Some(self.template_id));
        contact.last_updated_at = Set(Some(crate::db::now()));
        let contact = contact.update(db).await?;

        update_last_edited_date(db, contact).await
    }
}

/// `UpdateContactView`.
pub struct UpdateContactView {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl UpdateContactView {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::View))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let exists = contact_vault_user::Entity::find_by_id((self.contact_id, self.vault_id, self.author_id))
            .one(db)
            .await?
            .is_some();

        if exists {
            contact_vault_user::Entity::update_many()
                .col_expr(
                    contact_vault_user::Column::NumberOfViews,
                    Expr::col(contact_vault_user::Column::NumberOfViews).add(1),
                )
                .filter(contact_vault_user::Column::ContactId.eq(self.contact_id))
                .filter(contact_vault_user::Column::VaultId.eq(self.vault_id))
                .filter(contact_vault_user::Column::UserId.eq(self.author_id))
                .exec(db)
                .await?;
        } else {
            contact_vault_user::ActiveModel {
                contact_id: Set(self.contact_id),
                vault_id: Set(self.vault_id),
                user_id: Set(self.author_id),
                number_of_views: Set(1),
                created_at: Set(None),
                updated_at: Set(None),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }
}

/// `exists:genders,id` and `exists:pronouns,id`.
async fn exists_gender_and_pronoun<C: ConnectionTrait>(
    db: &C,
    v: &mut Validator,
    gender_id: Option<Id>,
    pronoun_id: Option<Id>,
) -> ApiResult<()> {
    if let Some(id) = gender_id
        && genders::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("gender_id", "exists", &[]);
    }
    if let Some(id) = pronoun_id
        && pronouns::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("pronoun_id", "exists", &[]);
    }
    Ok(())
}

/// `$this->account()->genders()->findOrFail($id)`.
async fn find_gender<C: ConnectionTrait>(db: &C, account_id: Uuid, id: Id) -> ApiResult<genders::Model> {
    genders::Entity::find_by_id(id)
        .filter(genders::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->account()->pronouns()->findOrFail($id)`.
async fn find_pronoun<C: ConnectionTrait>(db: &C, account_id: Uuid, id: Id) -> ApiResult<pronouns::Model> {
    pronouns::Entity::find_by_id(id)
        .filter(pronouns::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->account()->templates()->findOrFail($id)`.
async fn find_template<C: ConnectionTrait>(db: &C, account_id: Uuid, id: Id) -> ApiResult<templates::Model> {
    templates::Entity::find_by_id(id)
        .filter(templates::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `updateLastEditedDate`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: contacts::Model) -> ApiResult<contacts::Model> {
    let mut contact: contacts::ActiveModel = contact.into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    Ok(contact.update(db).await?)
}

/// `createFeedItem`.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    contact: &contacts::Model,
    action: &str,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact.id),
        action: Set(action.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

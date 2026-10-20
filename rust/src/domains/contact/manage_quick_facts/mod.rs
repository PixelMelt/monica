//! `app/Domains/Contact/ManageQuickFacts/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::{contacts, quick_facts, vault_quick_facts_templates};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateQuickFact`.
pub struct CreateQuickFact {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub vault_id: Uuid,
    pub vault_quick_facts_template_id: Id,
    pub content: String,
}

impl CreateQuickFact {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<quick_facts::Model> {
        self.validate(db).await?;

        let quick_fact = quick_facts::ActiveModel {
            vault_quick_facts_template_id: Set(self.vault_quick_facts_template_id),
            contact_id: Set(self.contact_id),
            content: Set(self.content),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(quick_fact)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        if vault_quick_facts_templates::Entity::find_by_id(self.vault_quick_facts_template_id).one(db).await?.is_none()
        {
            v.fail("vault_quick_facts_template_id", "exists", &[]);
        }
        content_rules(&mut v, &self.content);
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

        vault_quick_facts_templates::Entity::find_by_id(self.vault_quick_facts_template_id)
            .filter(vault_quick_facts_templates::Column::VaultId.eq(checked.vault().id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;
        Ok(())
    }
}

/// `DestroyQuickFact`.
pub struct DestroyQuickFact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub quick_fact_id: Id,
}

impl DestroyQuickFact {
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

        let quick_fact = find(db, checked.contact().id, self.quick_fact_id).await?;

        quick_facts::Entity::delete_by_id(quick_fact.id).exec(db).await?;
        Ok(())
    }
}

/// `ToggleQuickFactModule`.
pub struct ToggleQuickFactModule {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl ToggleQuickFactModule {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = self.validate(db).await?;
        Self::update(db, &checked).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<Checked> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await
    }

    async fn update<C: ConnectionTrait>(db: &C, checked: &Checked) -> ApiResult<()> {
        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.show_quick_facts = Set(!checked.contact().show_quick_facts);
        contact.update(db).await?;
        Ok(())
    }
}

/// `UpdateQuickFact`.
pub struct UpdateQuickFact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub quick_fact_id: Id,
    pub content: String,
}

impl UpdateQuickFact {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<quick_facts::Model> {
        let quick_fact = self.validate(db).await?;

        let mut quick_fact: quick_facts::ActiveModel = quick_fact.into();
        quick_fact.content = Set(self.content);
        Ok(quick_fact.update(db).await?)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<quick_facts::Model> {
        let mut v = Validator::new();
        content_rules(&mut v, &self.content);
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

        find(db, checked.contact().id, self.quick_fact_id).await
    }
}

/// `'content' => 'required|string|max:255'`.
fn content_rules(v: &mut Validator, content: &str) {
    if content.trim().is_empty() {
        v.fail("content", "required", &[]);
    }
    v.max_len("content", Some(content), 255);
}

/// `$this->contact->quickFacts()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, contact_id: Uuid, quick_fact_id: Id) -> ApiResult<quick_facts::Model> {
    quick_facts::Entity::find_by_id(quick_fact_id)
        .filter(quick_facts::Column::ContactId.eq(contact_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

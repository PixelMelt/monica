//! `app/Domains/Vault/ManageCompanies/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait};

use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::companies;
use crate::error::ApiResult;
use crate::validate::Validator;

/// `CreateCompany`.
pub struct CreateCompany {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub name: String,
    pub r#type: Option<String>,
}

impl CreateCompany {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<companies::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("type", self.r#type.as_deref(), 255);
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

        let company = companies::ActiveModel {
            vault_id: Set(self.vault_id),
            name: Set(self.name),
            r#type: Set(value_or_null(self.r#type)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(company)
    }
}

//! `app/Domains/Vault/ManageAddresses/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{address_types, addresses};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateAddress`.
pub struct CreateAddress {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub address_type_id: Option<Id>,
    pub line_1: Option<String>,
    pub line_2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl CreateAddress {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<addresses::Model> {
        let mut v = Validator::new();
        address_type_exists(db, &mut v, self.address_type_id).await?;
        v.max_len("line_1", self.line_1.as_deref(), 255);
        v.max_len("line_2", self.line_2.as_deref(), 255);
        v.max_len("city", self.city.as_deref(), 255);
        v.max_len("province", self.province.as_deref(), 255);
        v.max_len("postal_code", self.postal_code.as_deref(), 255);
        v.max_len("country", self.country.as_deref(), 255);
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

        if let Some(address_type_id) = self.address_type_id {
            find_address_type(db, self.account_id, address_type_id).await?;
        }

        let address = addresses::ActiveModel {
            vault_id: Set(self.vault_id),
            address_type_id: Set(self.address_type_id),
            line_1: Set(value_or_null(self.line_1)),
            line_2: Set(value_or_null(self.line_2)),
            city: Set(value_or_null(self.city)),
            province: Set(value_or_null(self.province)),
            postal_code: Set(value_or_null(self.postal_code)),
            country: Set(value_or_null(self.country)),
            latitude: Set(number_or_null(self.latitude)),
            longitude: Set(number_or_null(self.longitude)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(address)
    }
}

/// `DestroyAddress`.
pub struct DestroyAddress {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub address_id: Id,
}

impl DestroyAddress {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
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

        let address = find_address(db, self.vault_id, self.address_id).await?;

        addresses::Entity::delete_by_id(address.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateAddress`.
pub struct UpdateAddress {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub address_id: Id,
    pub address_type_id: Option<Id>,
    pub line_1: Option<String>,
    pub line_2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl UpdateAddress {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<addresses::Model> {
        let mut v = Validator::new();
        address_type_exists(db, &mut v, self.address_type_id).await?;
        v.max_len("line_1", self.line_1.as_deref(), 255);
        v.max_len("line_2", self.line_2.as_deref(), 255);
        v.max_len("city", self.city.as_deref(), 255);
        v.max_len("province", self.province.as_deref(), 255);
        v.max_len("postal_code", self.postal_code.as_deref(), 255);
        v.max_len("country", self.country.as_deref(), 255);
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

        if let Some(address_type_id) = self.address_type_id {
            find_address_type(db, self.account_id, address_type_id).await?;
        }

        let address = find_address(db, self.vault_id, self.address_id).await?;

        let mut address: addresses::ActiveModel = address.into();
        address.address_type_id = Set(self.address_type_id);
        address.line_1 = Set(value_or_null(self.line_1));
        address.line_2 = Set(value_or_null(self.line_2));
        address.city = Set(value_or_null(self.city));
        address.province = Set(value_or_null(self.province));
        address.postal_code = Set(value_or_null(self.postal_code));
        address.country = Set(value_or_null(self.country));
        address.latitude = Set(number_or_null(self.latitude));
        address.longitude = Set(number_or_null(self.longitude));
        Ok(address.update(db).await?)
    }
}

/// `exists:address_types,id`.
async fn address_type_exists<C: ConnectionTrait>(db: &C, v: &mut Validator, id: Option<Id>) -> ApiResult<()> {
    if let Some(id) = id
        && address_types::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("address_type_id", "exists", &[]);
    }
    Ok(())
}

/// `$this->account()->addressTypes()->findOrFail($id)`.
async fn find_address_type<C: ConnectionTrait>(db: &C, account_id: Uuid, id: Id) -> ApiResult<address_types::Model> {
    address_types::Entity::find_by_id(id)
        .filter(address_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->vault->addresses()->findOrFail($id)`.
async fn find_address<C: ConnectionTrait>(db: &C, vault_id: Uuid, id: Id) -> ApiResult<addresses::Model> {
    addresses::Entity::find_by_id(id)
        .filter(addresses::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `valueOrNull` on a number: `None` for 0.
fn number_or_null(value: Option<f64>) -> Option<f64> {
    value.filter(|v| *v != 0.0)
}

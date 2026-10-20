//! `app/Domains/Settings/ManageUsers/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::validate::Validator;

/// A username as stored: trimmed and in lower case, so signing in ignores
/// case on every database.
pub fn normalize_username(username: &str) -> String {
    username.trim().to_lowercase()
}

/// The rules of a (normalized) username: required, at most 255 characters,
/// no whitespace and no `:` (CardDAV clients send `username:token`), and no
/// other user's (`except` is the user being edited).
pub async fn validate_username<C: ConnectionTrait>(
    db: &C,
    v: &mut Validator,
    username: &str,
    except: Option<Uuid>,
) -> ApiResult<()> {
    if username.is_empty() {
        v.fail("username", "required", &[]);
        return Ok(());
    }
    v.max_len("username", Some(username), 255);
    if username.chars().any(|c| c.is_whitespace() || c == ':') {
        v.fail("username", "regex", &[]);
    }
    let taken = users::Entity::find().filter(users::Column::Username.eq(username)).one(db).await?;
    if taken.is_some_and(|u| Some(u.id) != except) {
        v.fail("username", "unique", &[]);
    }
    Ok(())
}

/// Adds a user to the account with a username and an initial password, set
/// by an administrator (in place of `InviteUser`'s email invitation).
pub struct CreateUser {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub username: String,
    pub password: String,
    pub is_administrator: bool,
}

impl CreateUser {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let username = normalize_username(&self.username);
        let mut v = Validator::new();
        for (field, value) in [("first_name", &self.first_name), ("last_name", &self.last_name)] {
            if value.trim().is_empty() {
                v.fail(field, "required", &[]);
            }
            v.max_len(field, Some(value), 255);
        }
        validate_username(db, &mut v, &username, None).await?;
        if self.password.is_empty() {
            v.fail("password", "required", &[]);
        }
        v.finish()?;

        let now = crate::db::now();
        Ok(users::ActiveModel {
            id: Set(Uuid::new()),
            account_id: Set(self.account_id),
            first_name: Set(Some(self.first_name)),
            last_name: Set(Some(self.last_name)),
            username: Set(username),
            password: Set(Some(crate::password::hash(&self.password))),
            is_account_administrator: Set(self.is_administrator),
            timezone: Set(Some("UTC".into())),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyUser`.
pub struct DestroyUser {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_id: Uuid,
}

impl DestroyUser {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let user = self.validate(db).await?;
        Self::destroy_all_vaults(db, &user).await?;
        Self::destroy(db, &user).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<users::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let user = find(db, self.account_id, self.user_id).await?;

        if self.user_id == self.author_id {
            return Err(ApiError::conflict("You can't delete yourself."));
        }
        Ok(user)
    }

    async fn destroy_all_vaults<C: ConnectionTrait>(db: &C, user: &users::Model) -> ApiResult<()> {
        let vaults_user_is_manager_of = user_vault::Entity::find()
            .filter(user_vault::Column::UserId.eq(user.id))
            .filter(user_vault::Column::Permission.eq(VaultPermission::Manage as i32))
            .all(db)
            .await?;

        for vault in vaults_user_is_manager_of {
            let other_manager = user_vault::Entity::find()
                .filter(user_vault::Column::VaultId.eq(vault.vault_id))
                .filter(user_vault::Column::UserId.ne(user.id))
                .filter(user_vault::Column::Permission.eq(VaultPermission::Manage as i32))
                .one(db)
                .await?;
            if other_manager.is_none() {
                vaults::Entity::delete_by_id(vault.vault_id).exec(db).await?;
            }
        }
        Ok(())
    }

    async fn destroy<C: ConnectionTrait>(db: &C, user: &users::Model) -> ApiResult<()> {
        users::Entity::delete_by_id(user.id).exec(db).await?;
        Ok(())
    }
}

/// `GiveAdministratorPrivilege`.
pub struct GiveAdministratorPrivilege {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_id: Uuid,
}

impl GiveAdministratorPrivilege {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let user = find(db, self.account_id, self.user_id).await?;

        let mut user: users::ActiveModel = user.into();
        user.is_account_administrator = Set(true);
        Ok(user.update(db).await?)
    }
}

/// `RemoveAdministratorPrivilege`.
pub struct RemoveAdministratorPrivilege {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_id: Uuid,
}

impl RemoveAdministratorPrivilege {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let user = find(db, self.account_id, self.user_id).await?;

        if user.id == checked.author.id {
            return Err(ApiError::conflict(i18n::t(
                &i18n::current(),
                "You cannot remove your own administrator privilege.",
                &[],
            )));
        }

        let mut user: users::ActiveModel = user.into();
        user.is_account_administrator = Set(false);
        Ok(user.update(db).await?)
    }
}

/// `$this->account()->users()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, user_id: Uuid) -> ApiResult<users::Model> {
    users::Entity::find_by_id(user_id)
        .filter(users::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

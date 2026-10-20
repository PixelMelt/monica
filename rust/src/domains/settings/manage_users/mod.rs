//! `app/Domains/Settings/ManageUsers/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::config::Config;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::domains::settings::manage_notification_channels::{CreateUserNotificationChannel, TYPE_EMAIL};
use crate::entities::{user_notification_channels, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::mail::{Email, Mailer};
use crate::validate::Validator;

/// `AcceptInvitation`.
pub struct AcceptInvitation {
    pub invitation_code: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub password: String,
}

impl AcceptInvitation {
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        mailer: &Mailer,
        config: &Config,
    ) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        for (field, value) in [("first_name", &self.first_name), ("last_name", &self.last_name)] {
            if value.trim().is_empty() {
                v.fail(field, "required", &[]);
            }
            v.max_len(field, Some(value), 255);
        }
        if self.password.trim().is_empty() {
            v.fail("password", "required", &[]);
        } else if !self.password.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            v.fail("password", "alpha_dash", &[]);
        }
        v.max_len("password", Some(&self.password), 255);
        v.finish()?;

        let user = self.find_user_by_invitation_code(db).await?;
        let user = self.update_user(db, user).await?;
        Self::create_notification_channel(db, mailer, config, &user).await?;

        Ok(user)
    }

    async fn find_user_by_invitation_code<C: ConnectionTrait>(&self, db: &C) -> ApiResult<users::Model> {
        users::Entity::find()
            .filter(users::Column::InvitationCode.eq(self.invitation_code.to_string()))
            .filter(users::Column::InvitationAcceptedAt.is_null())
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)
    }

    async fn update_user<C: ConnectionTrait>(&self, db: &C, user: users::Model) -> ApiResult<users::Model> {
        let mut user: users::ActiveModel = user.into();
        user.is_account_administrator = Set(false);
        user.first_name = Set(Some(self.first_name.clone()));
        user.last_name = Set(Some(self.last_name.clone()));
        user.invitation_accepted_at = Set(Some(crate::db::now()));
        user.email_verified_at = Set(Some(crate::db::now()));
        user.password = Set(Some(crate::password::hash(&self.password)));
        user.timezone = Set(Some("UTC".into()));
        Ok(user.update(db).await?)
    }

    async fn create_notification_channel<C: ConnectionTrait>(
        db: &C,
        mailer: &Mailer,
        config: &Config,
        user: &users::Model,
    ) -> ApiResult<()> {
        let channel = CreateUserNotificationChannel {
            account_id: user.account_id,
            author_id: user.id,
            label: "Email address".into(),
            r#type: TYPE_EMAIL.into(),
            content: user.email.clone(),
            verify_email: Some(false),
            preferred_time: Some("09:00".into()),
        }
        .execute(db, mailer, config)
        .await?;

        let mut channel: user_notification_channels::ActiveModel = channel.into();
        channel.verified_at = Set(Some(crate::db::now()));
        channel.active = Set(true);
        channel.update(db).await?;
        Ok(())
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

/// `InviteUser`.
pub struct InviteUser {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub email: String,
    pub is_administrator: bool,
}

impl InviteUser {
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        mailer: &Mailer,
        config: &Config,
    ) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        if self.email.trim().is_empty() {
            v.fail("email", "required", &[]);
        } else if users::Entity::find().filter(users::Column::Email.eq(&self.email)).one(db).await?.is_some() {
            v.fail("email", "unique", &[]);
        }
        v.max_len("email", Some(&self.email), 255);
        v.finish()?;
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

        let user = self.create_user(db).await?;
        Self::send_email(mailer, config, &user, &checked.author).await?;

        Ok(user)
    }

    async fn create_user<C: ConnectionTrait>(&self, db: &C) -> ApiResult<users::Model> {
        Ok(users::ActiveModel {
            id: Set(Uuid::new()),
            account_id: Set(self.account_id),
            email: Set(self.email.clone()),
            invitation_code: Set(Some(uuid::Uuid::new_v4().to_string())),
            is_account_administrator: Set(self.is_administrator),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }

    async fn send_email(mailer: &Mailer, config: &Config, user: &users::Model, author: &users::Model) -> ApiResult<()> {
        let l = i18n::current();
        let code = user.invitation_code.as_deref().expect("create_user sets the invitation code");
        let url = format!("{}/invitation/{code}", config.url);
        let user_name =
            format!("{} {}", author.first_name.as_deref().unwrap_or(""), author.last_name.as_deref().unwrap_or(""));
        let (text, html) = Mailer::render(
            &[
                i18n::t(&l, "Please join Monica", &[]),
                i18n::t(
                    &l,
                    ":UserName invites you to join Monica, an open source personal CRM, designed to help you document your relationships.",
                    &[("userName", &user_name)],
                ),
            ],
            Some((&i18n::t(&l, "Accept invitation and create your account", &[]), &url)),
            &[i18n::t(&l, "Thanks,", &[]), "Monica".into()],
        );
        mailer
            .send(Email {
                to: user.email.clone(),
                subject: i18n::t(&l, "You are invited to join Monica", &[]),
                text,
                html,
            })
            .await
            .map_err(ApiError::internal)
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

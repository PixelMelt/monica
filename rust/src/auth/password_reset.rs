//! Password reset (E006, V005–V007): Laravel's password broker with the
//! `password_resets` table, `expire` 60 minutes and `throttle` 60 seconds.
//! The table keeps the SHA-256 of the token (Laravel kept a bcrypt hash).

use chrono::Duration;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::config::Config;
use crate::crypto;
use crate::entities::{password_resets, users};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::mail::{Email, Mailer};

pub const EXPIRE_MINUTES: i64 = 60;
pub const THROTTLE_SECONDS: i64 = 60;

/// What `sendResetLink` answers.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkStatus {
    Sent,
    InvalidUser,
    Throttled,
}

pub async fn send_link<C: ConnectionTrait>(
    db: &C,
    mailer: &Mailer,
    config: &Config,
    email: &str,
) -> ApiResult<LinkStatus> {
    let Some(user) = users::Entity::find().filter(users::Column::Email.eq(email)).one(db).await? else {
        return Ok(LinkStatus::InvalidUser);
    };
    let now = crate::db::now();
    if let Some(existing) =
        password_resets::Entity::find().filter(password_resets::Column::Email.eq(email)).one(db).await?
        && existing.created_at.is_some_and(|t| now.0 - t.0 < Duration::seconds(THROTTLE_SECONDS))
    {
        return Ok(LinkStatus::Throttled);
    }
    password_resets::Entity::delete_many().filter(password_resets::Column::Email.eq(email)).exec(db).await?;
    let token = crypto::random_string(64);
    password_resets::ActiveModel {
        email: Set(email.to_owned()),
        token: Set(crypto::sha256_hex(token.as_bytes())),
        created_at: Set(Some(now)),
    }
    .insert(db)
    .await?;

    let l = i18n::current();
    let url = format!(
        "{}/reset-password/{token}?email={}",
        config.url,
        url::form_urlencoded::byte_serialize(email.as_bytes()).collect::<String>()
    );
    let (text, html) = Mailer::render(
        &[i18n::t(
            &l,
            "You are receiving this email because we received a password reset request for your account.",
            &[],
        )],
        Some((&i18n::t(&l, "Reset Password", &[]), &url)),
        &[
            i18n::t(
                &l,
                "This password reset link will expire in :count minutes.",
                &[("count", &EXPIRE_MINUTES.to_string())],
            ),
            i18n::t(&l, "If you did not request a password reset, no further action is required.", &[]),
        ],
    );
    mailer
        .send(Email { to: user.email, subject: i18n::t(&l, "Reset Password Notification", &[]), text, html })
        .await
        .map_err(ApiError::internal)?;
    Ok(LinkStatus::Sent)
}

/// The user whose valid, unexpired token this is; the token is deleted.
pub async fn consume<C: ConnectionTrait>(db: &C, email: &str, token: &str) -> ApiResult<Option<users::Model>> {
    let Some(row) = password_resets::Entity::find().filter(password_resets::Column::Email.eq(email)).one(db).await?
    else {
        return Ok(None);
    };
    let fresh = row.created_at.is_some_and(|t| crate::db::now().0 - t.0 < Duration::minutes(EXPIRE_MINUTES));
    if !fresh || !crypto::eq(&row.token, &crypto::sha256_hex(token.as_bytes())) {
        return Ok(None);
    }
    let user = users::Entity::find().filter(users::Column::Email.eq(email)).one(db).await?;
    if user.is_some() {
        password_resets::Entity::delete_many().filter(password_resets::Column::Email.eq(email)).exec(db).await?;
    }
    Ok(user)
}

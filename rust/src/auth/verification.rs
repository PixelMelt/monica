//! Email verification (E005, V010–V012): Laravel's `VerifyEmail`
//! notification and its signed link, `verification.expire` 60 minutes.
//!
//! The link opens the SPA at `/email/verify/{id}/{hash}?expires=&signature=`;
//! `hash` is the SHA-1 of the email (as Laravel), `signature` an HMAC of
//! the other three with `APP_KEY`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait};
use sha1::{Digest, Sha1};

use crate::config::Config;
use crate::crypto;
use crate::entities::users;
use crate::error::ApiResult;
use crate::i18n;
use crate::mail::{Email, Mailer};

pub const EXPIRE_MINUTES: i64 = 60;

pub fn email_hash(email: &str) -> String {
    hex::encode(Sha1::digest(email.as_bytes()))
}

fn payload(id: &str, hash: &str, expires: i64) -> String {
    format!("verification|{id}|{hash}|{expires}")
}

/// The SPA link for `user`, valid for an hour from `now` (unix seconds).
pub fn link(config: &Config, user: &users::Model, now: i64) -> String {
    let id = user.id.to_string();
    let hash = email_hash(&user.email);
    let expires = now + EXPIRE_MINUTES * 60;
    let signature = crypto::sign(&config.key, &payload(&id, &hash, expires));
    format!("{}/email/verify/{id}/{hash}?expires={expires}&signature={signature}", config.url)
}

/// Whether the link values are genuine, unexpired and for `user`.
pub fn valid(
    config: &Config,
    user: &users::Model,
    id: &str,
    hash: &str,
    expires: i64,
    signature: &str,
    now: i64,
) -> bool {
    crypto::eq(signature, &crypto::sign(&config.key, &payload(id, hash, expires)))
        && expires > now
        && id == user.id.to_string()
        && crypto::eq(hash, &email_hash(&user.email))
}

/// `User::sendEmailVerificationNotification`: mails the link, except that
/// with SMTP and no credentials the user is marked verified instead.
pub async fn send<C: ConnectionTrait>(
    db: &C,
    mailer: &Mailer,
    config: &Config,
    user: users::Model,
) -> ApiResult<users::Model> {
    if Mailer::smtp_without_credentials(config) {
        let mut u: users::ActiveModel = user.into();
        u.email_verified_at = Set(Some(crate::db::now()));
        return Ok(u.update(db).await?);
    }
    let l = i18n::current();
    let url = link(config, &user, chrono::Utc::now().timestamp());
    let (text, html) = Mailer::render(
        &[i18n::t(&l, "Please click the button below to verify your email address.", &[])],
        Some((&i18n::t(&l, "Verify Email Address", &[]), &url)),
        &[i18n::t(&l, "If you did not create an account, no further action is required.", &[])],
    );
    mailer
        .send(Email { to: user.email.clone(), subject: i18n::t(&l, "Verify Email Address", &[]), text, html })
        .await
        .map_err(crate::error::ApiError::internal)?;
    Ok(user)
}

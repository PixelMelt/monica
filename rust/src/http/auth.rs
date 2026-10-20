//! Extractors for the authenticated caller (api-design.md §7, §9).

use axum::extract::FromRequestParts;
use axum::http::Method;
use axum::http::request::Parts;

use super::session::{Credential, Identity, Session};
use super::state::AppState;
use crate::error::{ApiError, Code};

fn identity(parts: &Parts) -> Option<Identity> {
    parts.extensions.get::<Option<Identity>>().cloned().flatten()
}

fn check_ability(parts: &Parts, id: &Identity) -> Result<(), ApiError> {
    if let Credential::Token { abilities, .. } = &id.credential {
        let has = |a: &str| abilities.iter().any(|x| x == a || x == "*");
        let ok =
            if matches!(parts.method, Method::GET | Method::HEAD) { has("read") || has("write") } else { has("write") };
        if !ok {
            return Err(ApiError::new(Code::TokenAbilityMissing));
        }
    }
    Ok(())
}

/// An authenticated user with a verified email (most endpoints).
pub struct Auth(pub Identity);

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        let id = identity(parts).ok_or_else(ApiError::unauthenticated)?;
        check_ability(parts, &id)?;
        if id.user.email_verified_at.is_none() {
            return Err(ApiError::new(Code::EmailNotVerified));
        }
        Ok(Auth(id))
    }
}

/// An authenticated user, verified or not (bootstrap, email verification,
/// logout).
pub struct AuthUnverified(pub Identity);

impl FromRequestParts<AppState> for AuthUnverified {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        let id = identity(parts).ok_or_else(ApiError::unauthenticated)?;
        check_ability(parts, &id)?;
        Ok(AuthUnverified(id))
    }
}

/// An authenticated user with a verified email, through a cookie session
/// only: token management, so a token can't mint or widen tokens.
pub struct SessionAuth(pub Identity);

impl FromRequestParts<AppState> for SessionAuth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let Auth(id) = Auth::from_request_parts(parts, state).await?;
        if id.credential != Credential::Session {
            return Err(ApiError::forbidden().with_detail("Use a browser session for this action."));
        }
        Ok(SessionAuth(id))
    }
}

/// The caller if authenticated.
pub struct MaybeAuth(pub Option<Identity>);

impl FromRequestParts<AppState> for MaybeAuth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        Ok(MaybeAuth(identity(parts)))
    }
}

impl FromRequestParts<AppState> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Session>().cloned().ok_or_else(|| ApiError::internal("session layer missing"))
    }
}

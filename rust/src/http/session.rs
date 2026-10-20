//! Cookie sessions in the `sessions` table, CSRF, and resolving who is
//! calling (session user or bearer token), api-design.md §7.
//!
//! [`layer`] runs on every `/api/v1` request. It loads the session named by
//! the `monica_session` cookie, rejects unsafe cookie-authenticated requests
//! whose `X-XSRF-TOKEN` header doesn't match the session's CSRF token,
//! resolves the [`Identity`], runs the handler in the caller's locale, then
//! saves the session and sets the `monica_session` and `XSRF-TOKEN` cookies.

use std::sync::{Arc, Mutex};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use super::state::AppState;
use super::tokens;
use crate::crypto;
use crate::db::uuid::Uuid;
use crate::entities::{sessions, users};
use crate::error::{ApiError, Code};
use crate::i18n;

pub const SESSION_COOKIE: &str = "monica_session";
pub const XSRF_COOKIE: &str = "XSRF-TOKEN";
pub const XSRF_HEADER: &str = "x-xsrf-token";
/// Laravel's remember-me duration: 576000 minutes (400 days).
pub const REMEMBER_MINUTES: i64 = 576_000;
/// Fortify's `password_timeout`: 3 hours.
pub const PASSWORD_TIMEOUT_SECONDS: i64 = 10_800;

/// A login waiting for its 2FA code (Fortify's `login.id` / `login.remember`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingLogin {
    pub user_id: Uuid,
    pub remember: bool,
}

/// What the session stores in `sessions.payload`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub csrf: String,
    #[serde(default)]
    pub remember: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_confirmed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_login: Option<PendingLogin>,
}

#[derive(Debug, Default)]
struct State_ {
    /// The stored session's id, if one was loaded or saved.
    id: Option<String>,
    user_id: Option<Uuid>,
    payload: Payload,
    /// Must be written back.
    dirty: bool,
    /// The id to delete when the session is regenerated or destroyed.
    stale_id: Option<String>,
    destroyed: bool,
}

/// The request's session. Cheap to clone; changes are saved after the handler.
#[derive(Debug, Clone, Default)]
pub struct Session(Arc<Mutex<State_>>);

impl Session {
    fn lock(&self) -> std::sync::MutexGuard<'_, State_> {
        self.0.lock().unwrap()
    }

    pub fn exists(&self) -> bool {
        let s = self.lock();
        s.id.is_some() || s.dirty
    }

    pub fn user_id(&self) -> Option<Uuid> {
        self.lock().user_id
    }

    pub fn payload(&self) -> Payload {
        self.lock().payload.clone()
    }

    /// Starts a session if there is none (`GET /auth/csrf`).
    pub fn ensure(&self) {
        let mut s = self.lock();
        if s.id.is_none() && !s.dirty {
            s.payload.csrf = crypto::random_string(40);
            s.dirty = true;
        }
    }

    /// Logs `user` in: a new session id (no fixation), a new CSRF token.
    pub fn login(&self, user: Uuid, remember: bool) {
        let mut s = self.lock();
        s.stale_id = s.id.take();
        s.user_id = Some(user);
        s.payload = Payload { csrf: crypto::random_string(40), remember, ..Payload::default() };
        s.dirty = true;
        s.destroyed = false;
    }

    /// Logs out: the session row is deleted and its cookies cleared.
    pub fn destroy(&self) {
        let mut s = self.lock();
        s.stale_id = s.id.take();
        s.user_id = None;
        s.payload = Payload::default();
        s.dirty = false;
        s.destroyed = true;
    }

    pub fn set_pending_login(&self, pending: Option<PendingLogin>) {
        let mut s = self.lock();
        if s.payload.csrf.is_empty() {
            s.payload.csrf = crypto::random_string(40);
        }
        s.payload.pending_login = pending;
        s.dirty = true;
    }

    pub fn confirm_password(&self) {
        let mut s = self.lock();
        s.payload.password_confirmed_at = Some(Utc::now().timestamp());
        s.dirty = true;
    }

    /// Whether the password was confirmed in the last three hours.
    pub fn password_recently_confirmed(&self) -> bool {
        self.lock().payload.password_confirmed_at.is_some_and(|t| Utc::now().timestamp() - t < PASSWORD_TIMEOUT_SECONDS)
    }
}

/// How the caller authenticated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    Session,
    /// A personal access token: its id and abilities.
    Token {
        id: crate::db::id::Id,
        abilities: Vec<String>,
    },
}

/// The authenticated caller.
#[derive(Debug, Clone)]
pub struct Identity {
    pub user: users::Model,
    pub credential: Credential,
}

fn lifetime_minutes(state: &AppState, payload: &Payload) -> i64 {
    if payload.remember { REMEMBER_MINUTES } else { state.config.session_lifetime_minutes }
}

async fn load(state: &AppState, id: &str) -> Option<sessions::Model> {
    let row = sessions::Entity::find_by_id(id.to_owned()).one(&state.db).await.ok()??;
    let payload: Payload = serde_json::from_str(&row.payload).unwrap_or_default();
    let age_minutes = (Utc::now().timestamp() - i64::from(row.last_activity)) / 60;
    if age_minutes >= lifetime_minutes(state, &payload) {
        let _ = sessions::Entity::delete_by_id(id.to_owned()).exec(&state.db).await;
        return None;
    }
    Some(row)
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ").map(str::trim)
}

fn unsafe_method(m: &Method) -> bool {
    !matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn client_ip(headers: &HeaderMap, req: &Request) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_owned())
        .or_else(|| {
            req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip().to_string())
        })
}

pub async fn layer(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let jar = CookieJar::from_headers(req.headers());
    let session = Session::default();
    if let Some(cookie) = jar.get(SESSION_COOKIE)
        && let Some(row) = load(&state, cookie.value()).await
    {
        let mut s = session.lock();
        s.payload = serde_json::from_str(&row.payload).unwrap_or_default();
        s.user_id = row.user_id;
        s.id = Some(row.id);
        // Laravel writes the session on every request; touch it at most once a minute.
        s.dirty = Utc::now().timestamp() - i64::from(row.last_activity) >= 60;
    }

    let mut identity = None;
    if let Some(token) = bearer(req.headers()) {
        match tokens::authenticate(&state, token).await {
            Some(id) => identity = Some(id),
            None => return ApiError::unauthenticated().into_response(),
        }
    } else {
        if unsafe_method(req.method()) {
            let header = req.headers().get(XSRF_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default();
            let expected = session.payload().csrf;
            if expected.is_empty() || !crypto::eq(header, &expected) {
                return ApiError::new(Code::CsrfTokenMismatch).into_response();
            }
        }
        if let Some(user_id) = session.user_id() {
            match users::Entity::find_by_id(user_id).one(&state.db).await {
                Ok(Some(user)) => identity = Some(Identity { user, credential: Credential::Session }),
                Ok(None) => session.destroy(),
                Err(e) => return ApiError::from(e).into_response(),
            }
        }
    }

    let locale = match &identity {
        Some(i) if i18n::is_supported(&i.user.locale) => i.user.locale.clone(),
        _ => i18n::negotiate(req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok())).to_owned(),
    };
    let ip = client_ip(req.headers(), &req);
    let user_agent = req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(str::to_owned);
    req.extensions_mut().insert(session.clone());
    req.extensions_mut().insert(identity);
    let mut res = i18n::LOCALE.scope(locale, next.run(req)).await;

    if let Err(e) = persist(&state, &session, ip, user_agent, &mut res).await {
        return e.into_response();
    }
    res
}

async fn persist(
    state: &AppState,
    session: &Session,
    ip: Option<String>,
    user_agent: Option<String>,
    res: &mut Response,
) -> Result<(), ApiError> {
    let (stale, destroyed, dirty, id, user_id, payload) = {
        let mut s = session.lock();
        (s.stale_id.take(), s.destroyed, s.dirty, s.id.clone(), s.user_id, s.payload.clone())
    };
    if let Some(stale) = stale {
        sessions::Entity::delete_by_id(stale).exec(&state.db).await?;
    }
    let secure = state.config.secure_cookies;
    if destroyed {
        for name in [SESSION_COOKIE, XSRF_COOKIE] {
            let c = Cookie::build((name, "")).path("/").max_age(time::Duration::ZERO).build();
            append_cookie(res, &c);
        }
        return Ok(());
    }
    if !dirty {
        return Ok(());
    }
    let now = Utc::now().timestamp() as i32;
    let row = sessions::ActiveModel {
        id: Set(id.clone().unwrap_or_else(|| crypto::random_string(40))),
        user_id: Set(user_id),
        ip_address: Set(ip.map(|i| i.chars().take(45).collect())),
        user_agent: Set(user_agent),
        payload: Set(serde_json::to_string(&payload).unwrap_or_default()),
        last_activity: Set(now),
    };
    let new = id.is_none();
    let session_id = row.id.clone().unwrap();
    if new {
        row.insert(&state.db).await?;
        session.lock().id = Some(session_id.clone());
    } else {
        row.update(&state.db).await?;
    }
    let max_age = time::Duration::minutes(lifetime_minutes(state, &payload));
    let session_cookie = Cookie::build((SESSION_COOKIE, session_id))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .max_age(max_age)
        .build();
    let xsrf = Cookie::build((XSRF_COOKIE, payload.csrf))
        .path("/")
        .secure(secure)
        .same_site(SameSite::Lax)
        .max_age(max_age)
        .build();
    append_cookie(res, &session_cookie);
    append_cookie(res, &xsrf);
    Ok(())
}

fn append_cookie(res: &mut Response, cookie: &Cookie<'_>) {
    if let Ok(v) = HeaderValue::from_str(&cookie.encoded().to_string()) {
        res.headers_mut().append(header::SET_COOKIE, v);
    }
}

/// Deletes every other session of `user` (log out other browser sessions, V030).
pub async fn delete_other_sessions(state: &AppState, user: Uuid, keep: Option<&str>) -> Result<(), ApiError> {
    let mut q = sessions::Entity::delete_many().filter(sessions::Column::UserId.eq(user));
    if let Some(keep) = keep {
        q = q.filter(sessions::Column::Id.ne(keep.to_owned()));
    }
    q.exec(&state.db).await?;
    Ok(())
}

impl Session {
    /// The stored id, once saved.
    pub fn id(&self) -> Option<String> {
        self.lock().id.clone()
    }
}

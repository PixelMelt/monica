//! The CardDAV / CalDAV server (V038, I002, X024; api-design.md §11): `/dav`
//! as laravel-sabre's `DAVController` runs sabre/dav 4.7 with the nodes and
//! plugins of `DAVServiceProvider` (Auth, CardDAV, VCFExport, CalDAV,
//! ICSExport, Sync, ACL, `DAVRedirect`), behind the middleware of
//! `config/laravelsabre.php` (`EnsureDavRequestsAreStateful`,
//! `AuthenticateWithTokenOnBasicAuth`, `abilities:read,write`).
//!
//! [`server`] is `Sabre\DAV\Server` with `CorePlugin`; the other plugins
//! add their listeners in [`acl`], [`carddav`], [`caldav`] and [`sync`];
//! [`nodes`] is the tree and [`backend`] the Monica backends.

pub mod acl;
pub mod backend;
pub mod caldav;
pub mod carddav;
pub mod http;
pub mod nodes;
pub mod props;
pub mod server;
pub mod sync;
pub mod vobject_ext;
pub mod xml;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum_extra::extract::cookie::CookieJar;
use base64::Engine;

use crate::error::{ApiError, Code};
use crate::http::session::{self, Credential, Identity, Session};
use crate::http::state::AppState;
use crate::http::tokens;

/// The `/dav` and `/dav/*` routes.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/dav", any(handle))
        .route("/dav/", any(handle))
        .route("/dav/{*path}", any(handle))
        .layer(from_fn_with_state(state, authenticate))
}

/// `UnauthorizedHttpException('Basic', 'Invalid credentials.')`.
fn failed_basic_response() -> Response {
    let mut res = ApiError::unauthenticated().with_detail("Invalid credentials.").into_response();
    res.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Basic"));
    res
}

/// `EnsureDavRequestsAreStateful` and `AuthenticateWithTokenOnBasicAuth`: a
/// bearer token, or Basic with the user's email and a token as password;
/// without an `Authorization` header, the session cookie (and its CSRF check
/// on unsafe methods).
async fn authenticate(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let Some(authorization) =
        req.headers().get(header::AUTHORIZATION).map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
    else {
        if CookieJar::from_headers(req.headers()).get(session::SESSION_COOKIE).is_none() {
            return failed_basic_response();
        }
        return session::layer(State(state), req, next).await;
    };

    let identity = if let Some(token) = authorization.strip_prefix("Bearer ") {
        tokens::authenticate(&state, token.trim()).await
    } else if authorization.len() > 6 && authorization[..6].eq_ignore_ascii_case("basic ") {
        let decoded = base64::engine::general_purpose::STANDARD.decode(authorization[6..].trim()).unwrap_or_default();
        let decoded = String::from_utf8_lossy(&decoded).into_owned();
        match decoded.split_once(':') {
            // The PHP_AUTH_USER must match the user's email.
            Some((user, password)) => tokens::authenticate(&state, password).await.filter(|id| id.user.email == user),
            None => None,
        }
    } else {
        None
    };

    let Some(identity) = identity else {
        return failed_basic_response();
    };
    req.extensions_mut().insert(Some(identity));
    req.extensions_mut().insert(Session::default());
    next.run(req).await
}

/// `DAVController::init()`: the `abilities:read,write` check, then the server.
async fn handle(State(state): State<AppState>, req: Request) -> Response {
    let identity = req.extensions().get::<Option<Identity>>().cloned().flatten();
    let Some(identity) = identity else {
        return failed_basic_response();
    };
    if let Credential::Token { abilities, .. } = &identity.credential {
        for ability in ["read", "write"] {
            if !abilities.iter().any(|a| a == ability || a == "*") {
                return ApiError::new(Code::TokenAbilityMissing).into_response();
            }
        }
    }

    let (parts, body) = req.into_parts();
    let limit = usize::try_from(state.config.max_upload_size_mb * 1024 * 1024 + 64 * 1024).unwrap_or(usize::MAX);
    let Ok(bytes) = axum::body::to_bytes(body, limit).await else {
        return ApiError::new(Code::PayloadTooLarge).into_response();
    };
    // `DAV\StringUtil::ensureUTF8()`: ISO-8859-1 when it isn't UTF-8.
    let body = match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    };

    // laravel-sabre's Server::fullUrl(): the path with a trailing slash, then the query.
    let mut url = parts.uri.path().to_owned();
    if !url.ends_with('/') {
        url.push('/');
    }
    if let Some(query) = parts.uri.query() {
        url.push('?');
        url.push_str(query);
    }

    let request = server::Request { method: parts.method.as_str().to_owned(), url, headers: parts.headers, body };
    let ctx = backend::Ctx { db: &state.db, config: &state.config, user: identity.user };
    let mut server = server::Server::new(&ctx, request);
    crate::i18n::LOCALE.scope(state.config.locale.clone(), server.start()).await;

    to_response(server.response)
}

/// laravel-sabre's `getResponse()` and Symfony's `Response::prepare()`.
fn to_response(response: server::Response) -> Response {
    let status = response.status.unwrap_or(500);
    let mut headers = response.headers;
    let empty = status < 200 || status == 204 || status == 304;
    if empty {
        headers.retain(|(n, _)| !n.eq_ignore_ascii_case("Content-Type") && !n.eq_ignore_ascii_case("Content-Length"));
    } else {
        match headers.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case("Content-Type")) {
            None => headers.push(("Content-Type".to_owned(), vec!["text/html; charset=UTF-8".to_owned()])),
            Some((_, values)) => {
                let value = values.join(",");
                if value.to_lowercase().starts_with("text/") && !value.to_lowercase().contains("charset") {
                    *values = vec![format!("{value}; charset=UTF-8")];
                }
            }
        }
    }

    let body = if empty { String::new() } else { response.body.unwrap_or_default() };
    let mut res = (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body).into_response();
    let res_headers = res.headers_mut();
    res_headers.remove(header::CONTENT_TYPE);
    for (name, values) in headers {
        let Ok(name) = header::HeaderName::from_bytes(name.as_bytes()) else { continue };
        res_headers.remove(&name);
        for value in values {
            if let Ok(value) = HeaderValue::from_str(&value) {
                res_headers.append(&name, value);
            }
        }
    }
    res
}

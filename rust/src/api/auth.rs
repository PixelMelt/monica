//! `/auth`: sessions, first-run setup, 2FA challenge and password
//! confirmation (api-design.md §7.3).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::HeaderMap;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::schemas::CurrentUser;
use crate::auth::two_factor;
use crate::domains::settings::manage_users::{normalize_username, validate_username};
use crate::entities::users;
use crate::error::{ApiError, ApiResult};
use crate::http::auth::Auth;
use crate::http::json::Json;
use crate::http::session::{Credential, PASSWORD_TIMEOUT_SECONDS, PendingLogin, Session};
use crate::http::state::AppState;
use crate::i18n;
use crate::validate::{Validator, clean};

fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_owned())
        .unwrap_or_default()
}

#[utoipa::path(
    get,
    path = "/auth/csrf",
    operation_id = "getCsrfCookie",
    tag = "Auth",
    summary = "Prime the XSRF-TOKEN cookie",
    security(()),
    responses((status = 204, description = "Done; no body.")),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V037"])), ("x-services" = json!([])))
)]
pub async fn get_csrf_cookie(session: Session) -> StatusCode {
    session.ensure();
    StatusCode::NO_CONTENT
}

#[derive(Deserialize, ToSchema)]
pub struct LoginCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub remember: bool,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResult {
    /// True when the password was right and a 2FA code is now needed.
    pub two_factor: bool,
    /// The user, when logged in (no 2FA pending).
    pub user: Option<CurrentUser>,
}

#[utoipa::path(
    post,
    path = "/auth/login",
    operation_id = "logIn",
    tag = "Auth",
    summary = "Log in with username and password",
    security(()),
    request_body = LoginCredentials,
    responses(
        (status = 200, description = "Logged in, or waiting for the 2FA code.", body = LoginResult),
        (status = 403, description = "`csrf_token_mismatch`."),
        (status = 422, description = "Wrong credentials (on `username`)."),
        (status = 429, description = "5 attempts per minute per username and IP."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V002"])), ("x-services" = json!([])))
)]
pub async fn log_in(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<LoginCredentials>,
) -> ApiResult<Json<LoginResult>> {
    let username = clean(body.username).map(|u| normalize_username(&u));
    let mut v = Validator::new();
    v.required("username", &username);
    v.required("password", &body.password);
    v.finish()?;
    let (username, password) = (username.unwrap(), body.password.unwrap());

    let key = format!("login|{username}|{}", client_ip(&headers));
    state.limiter.per_minute(&key, 5)?;

    let user = users::Entity::find().filter(users::Column::Username.eq(username)).one(&state.db).await?;
    let Some(user) = user.filter(|u| u.password.as_deref().is_some_and(|h| crate::password::verify(&password, h)))
    else {
        let msg = i18n::group(&i18n::current(), "auth.failed", &[]).unwrap_or_default();
        return Err(ApiError::field("username", "failed", msg));
    };
    state.limiter.clear(&key);

    if two_factor::enabled(&user) {
        session.set_pending_login(Some(PendingLogin { user_id: user.id, remember: body.remember }));
        return Ok(Json(LoginResult { two_factor: true, user: None }));
    }
    session.login(user.id, body.remember);
    Ok(Json(LoginResult { two_factor: false, user: Some(user.into()) }))
}

#[derive(Deserialize, ToSchema)]
pub struct TwoFactorChallenge {
    pub code: Option<String>,
    pub recovery_code: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/two-factor-challenge",
    operation_id = "completeTwoFactorChallenge",
    tag = "Auth",
    summary = "Finish a login with a 2FA or recovery code",
    security(()),
    request_body = TwoFactorChallenge,
    responses(
        (status = 200, description = "Logged in.", body = CurrentUser),
        (status = 401, description = "No pending login in the session."),
        (status = 422, description = "Wrong code."),
        (status = 429, description = "5 attempts per minute per pending login."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V019"])), ("x-services" = json!([])))
)]
pub async fn complete_two_factor_challenge(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<TwoFactorChallenge>,
) -> ApiResult<Json<CurrentUser>> {
    let pending = session.payload().pending_login.ok_or_else(ApiError::unauthenticated)?;
    state.limiter.per_minute(&format!("two-factor|{}", pending.user_id), 5)?;
    let user =
        users::Entity::find_by_id(pending.user_id).one(&state.db).await?.ok_or_else(ApiError::unauthenticated)?;

    let config = &state.config;
    let l = i18n::current();
    if let Some(recovery) = clean(body.recovery_code) {
        let mut codes = two_factor::recovery_codes(config, &user);
        let Some(pos) = codes.iter().position(|c| crate::crypto::eq(c, &recovery)) else {
            return Err(ApiError::field(
                "recovery_code",
                "invalid",
                i18n::t(&l, "The provided two factor recovery code was invalid.", &[]),
            ));
        };
        codes[pos] = two_factor::new_recovery_code();
        let mut u: users::ActiveModel = user.clone().into();
        u.two_factor_recovery_codes = Set(Some(two_factor::encrypt_codes(config, &codes)));
        u.update(&state.db).await?;
    } else {
        let code = clean(body.code).unwrap_or_default();
        let secret = two_factor::secret(config, &user).unwrap_or_default();
        let now = chrono::Utc::now().timestamp() as u64;
        if !two_factor::verify(user.id, &secret, &user.username, &code, now) {
            return Err(ApiError::field(
                "code",
                "invalid",
                i18n::t(&l, "The provided two factor authentication code was invalid.", &[]),
            ));
        }
    }
    session.login(user.id, pending.remember);
    Ok(Json(user.into()))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    operation_id = "logOut",
    tag = "Auth",
    summary = "End the browser session",
    responses((status = 204, description = "Done; no body."), (status = 401, description = "Not logged in.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V003"])), ("x-services" = json!([])))
)]
pub async fn log_out(Auth(id): Auth, session: Session) -> StatusCode {
    if id.credential == Credential::Session {
        session.destroy();
    }
    StatusCode::NO_CONTENT
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordConfirmation {
    pub password: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/confirm-password",
    operation_id = "confirmPassword",
    tag = "Auth",
    summary = "Confirm the password for sensitive actions (3 hours)",
    request_body = PasswordConfirmation,
    responses((status = 204, description = "Confirmed."), (status = 422, description = "Wrong password (on `password`).")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V017"])), ("x-services" = json!([])))
)]
pub async fn confirm_password(
    Auth(id): Auth,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    if let Some(hash) = id.user.password.as_deref()
        && !crate::password::verify(body.password.as_deref().unwrap_or_default(), hash)
    {
        let msg = i18n::group(&i18n::current(), "auth.password", &[]).unwrap_or_default();
        return Err(ApiError::field("password", "password", msg));
    }
    session.confirm_password();
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct PasswordConfirmationStatus {
    pub confirmed: bool,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PasswordConfirmationStatusQuery {
    /// Window to check, in seconds (default 10800).
    #[param(minimum = 1)]
    pub seconds: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/auth/confirmed-password-status",
    operation_id = "getPasswordConfirmationStatus",
    tag = "Auth",
    summary = "Whether the password was confirmed in the last 3 hours",
    params(PasswordConfirmationStatusQuery),
    responses((status = 200, description = "The status.", body = PasswordConfirmationStatus)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V016"])), ("x-services" = json!([])))
)]
pub async fn get_password_confirmation_status(
    _auth: Auth,
    session: Session,
    Query(query): Query<PasswordConfirmationStatusQuery>,
) -> Json<PasswordConfirmationStatus> {
    let seconds = query.seconds.unwrap_or(PASSWORD_TIMEOUT_SECONDS);
    Json(PasswordConfirmationStatus { confirmed: session.password_confirmed_within(seconds) })
}

#[derive(Deserialize, ToSchema)]
pub struct Registration {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/register",
    operation_id = "registerUser",
    tag = "Auth",
    summary = "First run: create the account and its first user",
    description = "Only while no user exists (`GET /instance` says `setup_required`); later users are added by an administrator.",
    security(()),
    request_body = Registration,
    responses(
        (status = 201, description = "The new user, logged in.", body = CurrentUser),
        (status = 403, description = "`signup_disabled` (a user exists) or `csrf_token_mismatch`."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V009"])), ("x-services" = json!(["CreateNewUser", "CreateAccount", "SetupAccount"])))
)]
pub async fn register_user(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<Registration>,
) -> ApiResult<(StatusCode, Json<CurrentUser>)> {
    if !super::instance::setup_required(&state).await? {
        return Err(ApiError::new(crate::error::Code::SignupDisabled));
    }
    let first_name = clean(body.first_name);
    let last_name = clean(body.last_name);
    let username = normalize_username(body.username.as_deref().unwrap_or_default());
    let mut v = Validator::new();
    v.required("first_name", &first_name);
    v.required("last_name", &last_name);
    validate_username(&state.db, &mut v, &username, None).await?;
    if v.required("password", &body.password) {
        crate::password::validate(
            &mut v,
            "password",
            body.password.as_deref().unwrap_or_default(),
            state.config.is_production(),
            Some(body.password_confirmation.as_deref()),
        );
    }
    v.finish()?;

    let tx = state.db.begin().await?;
    let user = crate::domains::settings::create_account::CreateAccount {
        first_name: first_name.unwrap(),
        last_name: last_name.unwrap(),
        username,
        password: body.password,
        is_instance_administrator: true,
    }
    .execute(&tx)
    .await?;
    tx.commit().await?;
    session.login(user.id, false);
    Ok((StatusCode::CREATED, Json(user.into())))
}

//! `/auth`: sessions, registration, 2FA challenge, email verification,
//! password reset and confirmation (api-design.md §7.3).

use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::HeaderMap;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::schemas::CurrentUser;
use crate::auth::{password_reset, two_factor, verification};
use crate::entities::users;
use crate::error::{ApiError, ApiResult};
use crate::http::auth::AuthUnverified;
use crate::http::json::Json;
use crate::http::session::{Credential, PendingLogin, Session};
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
    pub email: Option<String>,
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
    summary = "Log in with email and password",
    security(()),
    request_body = LoginCredentials,
    responses(
        (status = 200, description = "Logged in, or waiting for the 2FA code.", body = LoginResult),
        (status = 403, description = "`csrf_token_mismatch`."),
        (status = 422, description = "Wrong credentials (on `email`)."),
        (status = 429, description = "5 attempts per minute per email and IP."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V002"])), ("x-services" = json!([])))
)]
pub async fn log_in(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(body): Json<LoginCredentials>,
) -> ApiResult<Json<LoginResult>> {
    let email = clean(body.email);
    let mut v = Validator::new();
    v.required("email", &email);
    v.required("password", &body.password);
    v.finish()?;
    let (email, password) = (email.unwrap(), body.password.unwrap());

    let key = format!("login|{}|{}", email.to_lowercase(), client_ip(&headers));
    state.limiter.per_minute(&key, 5)?;

    let user = users::Entity::find().filter(users::Column::Email.eq(email)).one(&state.db).await?;
    let Some(user) = user.filter(|u| u.password.as_deref().is_some_and(|h| crate::password::verify(&password, h)))
    else {
        let msg = i18n::group(&i18n::current(), "auth.failed", &[]).unwrap_or_default();
        return Err(ApiError::field("email", "failed", msg));
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
        if !two_factor::verify(user.id, &secret, &user.email, &code, now) {
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
pub async fn log_out(AuthUnverified(id): AuthUnverified, session: Session) -> StatusCode {
    if id.credential == Credential::Session {
        session.destroy();
    }
    StatusCode::NO_CONTENT
}

#[derive(Deserialize, ToSchema)]
pub struct EmailVerification {
    pub id: Option<String>,
    pub hash: Option<String>,
    pub expires: Option<i64>,
    pub signature: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/email/verify",
    operation_id = "verifyEmail",
    tag = "Auth",
    summary = "Verify the email address from the emailed link",
    request_body = EmailVerification,
    responses(
        (status = 204, description = "Verified."),
        (status = 403, description = "Bad, expired or someone else's link."),
        (status = 429, description = "6 per minute."),
    ),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V011"])), ("x-services" = json!([])))
)]
pub async fn verify_email(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    Json(body): Json<EmailVerification>,
) -> ApiResult<StatusCode> {
    state.limiter.per_minute(&format!("verification|{}", id.user.id), 6)?;
    let now = chrono::Utc::now().timestamp();
    let ok = verification::valid(
        &state.config,
        &id.user,
        body.id.as_deref().unwrap_or_default(),
        body.hash.as_deref().unwrap_or_default(),
        body.expires.unwrap_or_default(),
        body.signature.as_deref().unwrap_or_default(),
        now,
    );
    if !ok {
        return Err(ApiError::forbidden().with_detail("Invalid or expired verification link."));
    }
    if id.user.email_verified_at.is_none() {
        let mut u: users::ActiveModel = id.user.into();
        u.email_verified_at = Set(Some(crate::db::now()));
        u.update(&state.db).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/email/verification-notification",
    operation_id = "sendEmailVerification",
    tag = "Auth",
    summary = "Email the verification link again",
    responses((status = 204, description = "Sent (or already verified)."), (status = 429, description = "6 per minute.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V012"])), ("x-services" = json!([])))
)]
pub async fn send_email_verification(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
) -> ApiResult<StatusCode> {
    state.limiter.per_minute(&format!("verification|{}", id.user.id), 6)?;
    if id.user.email_verified_at.is_none() {
        verification::send(&state.db, &state.mailer, &state.config, id.user).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordResetLinkRequest {
    pub email: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/forgot-password",
    operation_id = "requestPasswordReset",
    tag = "Auth",
    summary = "Email a password reset link",
    security(()),
    request_body = PasswordResetLinkRequest,
    responses(
        (status = 204, description = "Sent if the email has an account (the answer is the same either way)."),
        (status = 422, description = "A link was sent less than 60 seconds ago."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V005"])), ("x-services" = json!([])))
)]
pub async fn request_password_reset(
    State(state): State<AppState>,
    Json(body): Json<PasswordResetLinkRequest>,
) -> ApiResult<StatusCode> {
    let email = clean(body.email);
    let mut v = Validator::new();
    if v.required("email", &email) {
        v.email("email", email.as_deref());
    }
    v.finish()?;
    let l = i18n::current();
    // Q8: an unknown email gets the same answer as a known one, so the
    // endpoint doesn't reveal who has an account.
    match password_reset::send_link(&state.db, &state.mailer, &state.config, &email.unwrap()).await? {
        password_reset::LinkStatus::Sent | password_reset::LinkStatus::InvalidUser => Ok(StatusCode::NO_CONTENT),
        password_reset::LinkStatus::Throttled => {
            Err(ApiError::field("email", "throttled", i18n::group(&l, "passwords.throttled", &[]).unwrap_or_default()))
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordReset {
    pub token: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
}

#[utoipa::path(
    post,
    path = "/auth/reset-password",
    operation_id = "resetPassword",
    tag = "Auth",
    summary = "Set a new password with the emailed token",
    security(()),
    request_body = PasswordReset,
    responses((status = 204, description = "Password changed."), (status = 422, description = "Bad or expired token, or password rules.")),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V007"])), ("x-services" = json!(["ResetUserPassword"])))
)]
pub async fn reset_password(State(state): State<AppState>, Json(body): Json<PasswordReset>) -> ApiResult<StatusCode> {
    let email = clean(body.email);
    let token = clean(body.token);
    let mut v = Validator::new();
    v.required("token", &token);
    if v.required("email", &email) {
        v.email("email", email.as_deref());
    }
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
    let Some(user) = password_reset::consume(&state.db, &email.unwrap(), &token.unwrap()).await? else {
        let msg = i18n::group(&i18n::current(), "passwords.token", &[]).unwrap_or_default();
        return Err(ApiError::field("email", "token", msg));
    };
    let user_id = user.id;
    let mut u: users::ActiveModel = user.into();
    u.password = Set(Some(crate::password::hash(&body.password.unwrap())));
    u.remember_token = Set(Some(crate::crypto::random_string(60)));
    u.update(&state.db).await?;
    crate::http::session::delete_other_sessions(&state, user_id, None).await?;
    Ok(StatusCode::NO_CONTENT)
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
    AuthUnverified(id): AuthUnverified,
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

#[utoipa::path(
    get,
    path = "/auth/confirmed-password-status",
    operation_id = "getPasswordConfirmationStatus",
    tag = "Auth",
    summary = "Whether the password was confirmed in the last 3 hours",
    responses((status = 200, description = "The status.", body = PasswordConfirmationStatus)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V016"])), ("x-services" = json!([])))
)]
pub async fn get_password_confirmation_status(
    _auth: AuthUnverified,
    session: Session,
) -> Json<PasswordConfirmationStatus> {
    Json(PasswordConfirmationStatus { confirmed: session.password_recently_confirmed() })
}

#[derive(Deserialize, ToSchema)]
pub struct Registration {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
    /// The terms of service and privacy policy are accepted.
    pub terms: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/auth/register",
    operation_id = "registerUser",
    tag = "Auth",
    summary = "Create an account and its first user",
    security(()),
    request_body = Registration,
    responses(
        (status = 201, description = "The new user, logged in.", body = CurrentUser),
        (status = 403, description = "`signup_disabled` or `csrf_token_mismatch`."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V009"])), ("x-services" = json!(["CreateNewUser", "CreateAccount", "SetupAccount"])))
)]
pub async fn register_user(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<Registration>,
) -> ApiResult<(StatusCode, Json<CurrentUser>)> {
    if !super::instance::signup_enabled(&state).await? {
        return Err(ApiError::new(crate::error::Code::SignupDisabled));
    }
    // CreateNewUser's rules; CreateAccount checks the rest again.
    let first_name = clean(body.first_name);
    let last_name = clean(body.last_name);
    let email = clean(body.email);
    let mut v = Validator::new();
    v.required("first_name", &first_name);
    v.required("last_name", &last_name);
    v.required("email", &email);
    if v.required("password", &body.password) && body.password_confirmation.as_deref() != body.password.as_deref() {
        v.fail("password", "confirmed", &[]);
    }
    if body.terms != Some(true) {
        v.fail("terms", "accepted", &[]);
    }
    v.finish()?;

    let tx = state.db.begin().await?;
    let user = crate::domains::settings::create_account::CreateAccount {
        first_name: first_name.unwrap(),
        last_name: last_name.unwrap(),
        email: email.unwrap(),
        password: body.password,
        storage_limit_in_mb: state.config.default_storage_limit_mb,
    }
    .execute(&tx, &state.mailer, &state.config)
    .await?;
    tx.commit().await?;
    let user = verification::send(&state.db, &state.mailer, &state.config, user).await?;
    session.login(user.id, false);
    Ok((StatusCode::CREATED, Json(user.into())))
}

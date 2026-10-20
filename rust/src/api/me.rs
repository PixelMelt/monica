//! `/me`: the logged-in user's profile, password, sessions, tokens and 2FA.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::TransactionTrait;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::auth::PasswordConfirmation;
use super::instance::{Instance, build_instance};
use super::schemas::{CurrentUser, UserPreferences};
use crate::auth::{two_factor, verification};
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::entities::{password_resets, personal_access_tokens, sessions, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::http::auth::{AuthUnverified, SessionAuth, require_password_confirmation};
use crate::http::json::Json;
use crate::http::session::{Session, delete_other_sessions};
use crate::http::state::AppState;
use crate::http::tokens;
use crate::i18n;
use crate::validate::{Validator, clean};

#[utoipa::path(
    get,
    path = "/me",
    operation_id = "getMe",
    tag = "Me",
    summary = "The logged-in user",
    responses((status = 200, description = "The user.", body = CurrentUser), (status = 401, description = "Not logged in.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R329", "V029"])), ("x-services" = json!([])))
)]
pub async fn get_me(AuthUnverified(id): AuthUnverified) -> Json<CurrentUser> {
    Json(id.user.into())
}

#[derive(Deserialize, ToSchema)]
pub struct CurrentUserUpdate {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
}

#[utoipa::path(
    patch,
    path = "/me",
    operation_id = "updateMe",
    tag = "Me",
    summary = "Change name or email",
    request_body = CurrentUserUpdate,
    responses((status = 200, description = "The user.", body = CurrentUser), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V013"])), ("x-services" = json!(["UpdateUserProfileInformation"])))
)]
pub async fn update_me(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    Json(body): Json<CurrentUserUpdate>,
) -> ApiResult<Json<CurrentUser>> {
    let user = id.user;
    let first_name = clean(body.first_name).or(user.first_name.clone());
    let last_name = clean(body.last_name).or(user.last_name.clone());
    let email = clean(body.email).unwrap_or_else(|| user.email.clone());
    let mut v = Validator::new();
    v.required("first_name", &first_name);
    v.max_len("first_name", first_name.as_deref(), 255);
    v.required("last_name", &last_name);
    v.max_len("last_name", last_name.as_deref(), 255);
    v.email("email", Some(&email));
    v.max_len("email", Some(&email), 255);
    if email != user.email
        && users::Entity::find().filter(users::Column::Email.eq(&email)).one(&state.db).await?.is_some()
    {
        v.fail("email", "unique", &[]);
    }
    v.finish()?;

    let changed_email = email != user.email;
    let mut u = user.into_active_model();
    u.first_name = Set(first_name);
    u.last_name = Set(last_name);
    u.email = Set(email);
    if changed_email {
        u.email_verified_at = Set(None);
    }
    let mut user = u.update(&state.db).await?;
    if changed_email {
        user = verification::send(&state.db, &state.mailer, &state.config, user).await?;
    }
    Ok(Json(user.into()))
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordUpdate {
    pub current_password: Option<String>,
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
}

#[utoipa::path(
    put,
    path = "/me/password",
    operation_id = "updateMyPassword",
    tag = "Me",
    summary = "Change the password",
    request_body = PasswordUpdate,
    responses((status = 204, description = "Changed."), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V014"])), ("x-services" = json!(["UpdateUserPassword"])))
)]
pub async fn update_my_password(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
    Json(body): Json<PasswordUpdate>,
) -> ApiResult<StatusCode> {
    let mut v = Validator::new();
    v.required("current_password", &body.current_password);
    if v.required("password", &body.password) {
        crate::password::validate(
            &mut v,
            "password",
            body.password.as_deref().unwrap_or_default(),
            state.config.is_production(),
            Some(body.password_confirmation.as_deref()),
        );
    }
    let current_ok = body
        .current_password
        .as_deref()
        .zip(id.user.password.as_deref())
        .is_some_and(|(p, h)| crate::password::verify(p, h));
    if !current_ok && !v.has("current_password") {
        let msg = i18n::t(&i18n::current(), "The provided password does not match your current password.", &[]);
        v.add("current_password", "current_password", msg);
    }
    v.finish()?;

    let user_id = id.user.id;
    let email = id.user.email.clone();
    let mut u = id.user.into_active_model();
    u.password = Set(Some(crate::password::hash(&body.password.unwrap())));
    u.update(&state.db).await?;
    password_resets::Entity::delete_many().filter(password_resets::Column::Email.eq(email)).exec(&state.db).await?;
    delete_other_sessions(&state, user_id, session.id().as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// What the layout shows for a vault (today `layoutData.vault`).
#[derive(Serialize, ToSchema)]
pub struct BootstrapVault {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    pub name: String,
    /// The user's permission in the vault (100 manage, 200 edit, 300 view).
    pub permission: i32,
    pub show_group_tab: bool,
    pub show_tasks_tab: bool,
    pub show_files_tab: bool,
    pub show_journal_tab: bool,
    pub show_companies_tab: bool,
    pub show_reports_tab: bool,
    pub show_calendar_tab: bool,
}

#[derive(Serialize, ToSchema)]
pub struct Bootstrap {
    pub user: CurrentUser,
    pub vaults: Vec<BootstrapVault>,
    pub instance: Instance,
}

#[utoipa::path(
    get,
    path = "/me/bootstrap",
    operation_id = "getBootstrap",
    tag = "Me",
    summary = "Everything the layout needs (aggregate)",
    responses((status = 200, description = "The bootstrap data.", body = Bootstrap)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!([])), ("x-services" = json!([])))
)]
pub async fn get_bootstrap(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
) -> ApiResult<Json<Bootstrap>> {
    let rows = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(id.user.id))
        .find_also_related(vaults::Entity)
        .all(&state.db)
        .await?;
    let mut vaults: Vec<BootstrapVault> = rows
        .into_iter()
        .filter_map(|(m, v)| v.map(|v| (m, v)))
        .map(|(m, v)| BootstrapVault {
            id: v.id,
            name: v.name,
            permission: m.permission,
            show_group_tab: v.show_group_tab,
            show_tasks_tab: v.show_tasks_tab,
            show_files_tab: v.show_files_tab,
            show_journal_tab: v.show_journal_tab,
            show_companies_tab: v.show_companies_tab,
            show_reports_tab: v.show_reports_tab,
            show_calendar_tab: v.show_calendar_tab,
        })
        .collect();
    vaults.sort_by_key(|v| v.name.to_lowercase());
    Ok(Json(Bootstrap { instance: build_instance(&state).await?, user: id.user.into(), vaults }))
}

#[utoipa::path(
    get,
    path = "/me/preferences",
    operation_id = "getMyPreferences",
    tag = "Me",
    summary = "Display and format preferences",
    responses((status = 200, description = "The preferences.", body = UserPreferences)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R212"])), ("x-services" = json!([])))
)]
pub async fn get_my_preferences(AuthUnverified(id): AuthUnverified) -> Json<UserPreferences> {
    Json(CurrentUser::from(id.user).preferences)
}

/// A browser session of the user (Jetstream's "Browser Sessions").
#[derive(Serialize, ToSchema)]
pub struct SessionInfo {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    /// Computed from `user_agent`.
    pub is_desktop: bool,
    /// Computed from `user_agent`; null when unknown.
    pub platform: Option<String>,
    /// Computed from `user_agent`; null when unknown.
    pub browser: Option<String>,
    /// This request's session.
    pub is_current_device: bool,
    #[schema(value_type = String, format = DateTime)]
    pub last_activity: Timestamp,
}

#[derive(Serialize, ToSchema)]
pub struct SessionList {
    pub data: Vec<SessionInfo>,
}

/// Platform, browser and desktop-ness of a user agent, like Jetstream's
/// `Agent` (jenssegers/agent) for the common cases.
pub fn describe_agent(ua: &str) -> (Option<String>, Option<String>, bool) {
    let has = |s: &str| ua.contains(s);
    let platform = if has("iPhone") || has("iPad") || has("iPod") {
        Some("iOS")
    } else if has("Android") {
        Some("AndroidOS")
    } else if has("Windows") {
        Some("Windows")
    } else if has("CrOS") {
        Some("ChromeOS")
    } else if has("Macintosh") || has("Mac OS X") {
        Some("OS X")
    } else if has("Linux") {
        Some("Linux")
    } else {
        None
    };
    let browser = if has("Edg/") || has("Edge/") {
        Some("Edge")
    } else if has("OPR/") || has("Opera") {
        Some("Opera")
    } else if has("Firefox/") {
        Some("Firefox")
    } else if has("Chrome/") || has("CriOS/") {
        Some("Chrome")
    } else if has("Safari/") {
        Some("Safari")
    } else {
        None
    };
    let mobile = has("Mobile") || has("Android") || has("iPhone") || has("iPad") || has("iPod");
    (platform.map(str::to_owned), browser.map(str::to_owned), !mobile && platform.is_some())
}

#[utoipa::path(
    get,
    path = "/me/sessions",
    operation_id = "listMySessions",
    tag = "Me",
    summary = "The user's browser sessions, most recent first",
    responses((status = 200, description = "The sessions.", body = SessionList)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V029"])), ("x-services" = json!([])))
)]
pub async fn list_my_sessions(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
) -> ApiResult<Json<SessionList>> {
    let current = session.id();
    let rows = sessions::Entity::find()
        .filter(sessions::Column::UserId.eq(id.user.id))
        .order_by_desc(sessions::Column::LastActivity)
        .all(&state.db)
        .await?;
    let data = rows
        .into_iter()
        .map(|r| {
            let (platform, browser, is_desktop) = describe_agent(r.user_agent.as_deref().unwrap_or_default());
            SessionInfo {
                is_current_device: current.as_deref() == Some(r.id.as_str()),
                ip_address: r.ip_address,
                user_agent: r.user_agent,
                is_desktop,
                platform,
                browser,
                last_activity: chrono::DateTime::from_timestamp(i64::from(r.last_activity), 0)
                    .map(Timestamp::from)
                    .unwrap_or_else(crate::db::now),
            }
        })
        .collect();
    Ok(Json(SessionList { data }))
}

#[utoipa::path(
    post,
    path = "/me/sessions/log-out-others",
    operation_id = "logOutOtherSessions",
    tag = "Me",
    summary = "Log out every other browser session",
    request_body = PasswordConfirmation,
    responses((status = 204, description = "Done."), (status = 403, description = "`password_confirmation_required`.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V030"])), ("x-services" = json!([])))
)]
pub async fn log_out_other_sessions(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    delete_other_sessions(&state, id.user.id, session.id().as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- tokens

#[derive(Serialize, ToSchema)]
pub struct PersonalAccessToken {
    #[schema(value_type = i64)]
    pub id: Id,
    pub name: String,
    pub abilities: Vec<String>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_used_at: Option<Timestamp>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub expires_at: Option<Timestamp>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

impl From<personal_access_tokens::Model> for PersonalAccessToken {
    fn from(t: personal_access_tokens::Model) -> Self {
        PersonalAccessToken {
            abilities: tokens::abilities_of(&t),
            id: t.id,
            name: t.name,
            last_used_at: t.last_used_at,
            expires_at: t.expires_at,
            created_at: t.created_at.unwrap_or_else(crate::db::now),
            updated_at: t.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct PersonalAccessTokenList {
    pub data: Vec<PersonalAccessToken>,
}

#[utoipa::path(
    get,
    path = "/me/tokens",
    operation_id = "listMyTokens",
    tag = "Me",
    summary = "The user's API tokens",
    responses((status = 200, description = "The tokens.", body = PersonalAccessTokenList)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V033"])), ("x-services" = json!([])))
)]
pub async fn list_my_tokens(
    State(state): State<AppState>,
    crate::http::auth::Auth(id): crate::http::auth::Auth,
) -> ApiResult<Json<PersonalAccessTokenList>> {
    let rows = personal_access_tokens::Entity::find()
        .filter(personal_access_tokens::Column::TokenableType.eq(tokens::TOKENABLE_TYPE))
        .filter(personal_access_tokens::Column::TokenableId.eq(id.user.id))
        .order_by_asc(personal_access_tokens::Column::CreatedAt)
        .order_by_asc(personal_access_tokens::Column::Id)
        .all(&state.db)
        .await?;
    Ok(Json(PersonalAccessTokenList { data: rows.into_iter().map(Into::into).collect() }))
}

#[derive(Deserialize, ToSchema)]
pub struct PersonalAccessTokenCreate {
    pub name: Option<String>,
    /// `read` and/or `write`; default `read` (Jetstream's default permissions).
    pub abilities: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub struct PersonalAccessTokenCreated {
    pub token: PersonalAccessToken,
    /// Shown once.
    pub plain_text_token: String,
}

fn check_abilities(v: &mut Validator, abilities: &[String]) {
    for (i, a) in abilities.iter().enumerate() {
        if !tokens::ABILITIES.contains(&a.as_str()) {
            v.fail(&format!("abilities.{i}"), "in", &[]);
        }
    }
}

#[utoipa::path(
    post,
    path = "/me/tokens",
    operation_id = "createMyToken",
    tag = "Me",
    summary = "Create an API token",
    request_body = PersonalAccessTokenCreate,
    responses(
        (status = 201, description = "Created; the plain token is shown once.", body = PersonalAccessTokenCreated),
        (status = 403, description = "Bearer tokens can't create tokens."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V034"])), ("x-services" = json!([])))
)]
pub async fn create_my_token(
    State(state): State<AppState>,
    SessionAuth(id): SessionAuth,
    Json(body): Json<PersonalAccessTokenCreate>,
) -> ApiResult<(StatusCode, Json<PersonalAccessTokenCreated>)> {
    let name = clean(body.name);
    let abilities = body.abilities.unwrap_or_else(|| vec!["read".into()]);
    let mut v = Validator::new();
    v.required("name", &name);
    v.max_len("name", name.as_deref(), 255);
    check_abilities(&mut v, &abilities);
    v.finish()?;
    let (row, plain) = tokens::create(&state, id.user.id, &name.unwrap(), &abilities).await?;
    Ok((StatusCode::CREATED, Json(PersonalAccessTokenCreated { token: row.into(), plain_text_token: plain })))
}

#[derive(Deserialize, ToSchema)]
pub struct PersonalAccessTokenUpdate {
    pub abilities: Option<Vec<String>>,
}

async fn my_token(state: &AppState, user: Uuid, token_id: Id) -> ApiResult<personal_access_tokens::Model> {
    personal_access_tokens::Entity::find_by_id(token_id)
        .filter(personal_access_tokens::Column::TokenableType.eq(tokens::TOKENABLE_TYPE))
        .filter(personal_access_tokens::Column::TokenableId.eq(user))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

#[utoipa::path(
    patch,
    path = "/me/tokens/{token_id}",
    operation_id = "updateMyToken",
    tag = "Me",
    summary = "Change a token's abilities",
    params(("token_id" = i64, Path)),
    request_body = PersonalAccessTokenUpdate,
    responses((status = 200, description = "The token.", body = PersonalAccessToken), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V035"])), ("x-services" = json!([])))
)]
pub async fn update_my_token(
    State(state): State<AppState>,
    SessionAuth(id): SessionAuth,
    Path(token_id): Path<i64>,
    Json(body): Json<PersonalAccessTokenUpdate>,
) -> ApiResult<Json<PersonalAccessToken>> {
    let token = my_token(&state, id.user.id, Id(token_id)).await?;
    let mut v = Validator::new();
    v.required("abilities", &body.abilities);
    let abilities = body.abilities.unwrap_or_default();
    check_abilities(&mut v, &abilities);
    v.finish()?;
    let mut t = token.into_active_model();
    t.abilities = Set(Some(serde_json::to_string(&abilities).unwrap_or_else(|_| "[]".into())));
    Ok(Json(t.update(&state.db).await?.into()))
}

#[utoipa::path(
    delete,
    path = "/me/tokens/{token_id}",
    operation_id = "deleteMyToken",
    tag = "Me",
    summary = "Revoke a token",
    params(("token_id" = i64, Path)),
    responses((status = 204, description = "Revoked."), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V036"])), ("x-services" = json!([])))
)]
pub async fn delete_my_token(
    State(state): State<AppState>,
    crate::http::auth::Auth(id): crate::http::auth::Auth,
    Path(token_id): Path<i64>,
) -> ApiResult<StatusCode> {
    let token = my_token(&state, id.user.id, Id(token_id)).await?;
    personal_access_tokens::Entity::delete_by_id(token.id).exec(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- two-factor

#[derive(Serialize, ToSchema)]
pub struct TwoFactorSetup {
    pub secret: String,
    pub otpauth_url: String,
    pub qr_code_svg: String,
    pub recovery_codes: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TwoFactorRecoveryCodes {
    pub recovery_codes: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/me/two-factor",
    operation_id = "enableTwoFactor",
    tag = "Me",
    summary = "Start enabling 2FA",
    request_body = PasswordConfirmation,
    responses(
        (status = 200, description = "The new secret and recovery codes.", body = TwoFactorSetup),
        (status = 403, description = "`password_confirmation_required`."),
        (status = 409, description = "2FA is already confirmed."),
    ),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V020", "V023", "V024"])), ("x-services" = json!(["EnableTwoFactorAuthentication"])))
)]
pub async fn enable_two_factor(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<Json<TwoFactorSetup>> {
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    if two_factor::enabled(&id.user) {
        return Err(ApiError::conflict("Two-factor authentication is already enabled."));
    }
    let secret = two_factor::new_secret();
    let codes = two_factor::new_recovery_codes();
    let otpauth_url =
        two_factor::otpauth_url(&secret, &id.user.email).ok_or_else(|| ApiError::internal("otpauth url"))?;
    let qr_code_svg = two_factor::qr_code_svg(&otpauth_url).ok_or_else(|| ApiError::internal("qr code"))?;
    let mut u = id.user.into_active_model();
    u.two_factor_secret = Set(Some(crate::crypto::encrypt(&state.config.key, &secret)));
    u.two_factor_recovery_codes = Set(Some(two_factor::encrypt_codes(&state.config, &codes)));
    u.two_factor_confirmed_at = Set(None);
    u.update(&state.db).await?;
    Ok(Json(TwoFactorSetup { secret, otpauth_url, qr_code_svg, recovery_codes: codes }))
}

#[derive(Deserialize, ToSchema)]
pub struct TwoFactorConfirm {
    pub code: Option<String>,
}

#[utoipa::path(
    post,
    path = "/me/two-factor/confirm",
    operation_id = "confirmTwoFactor",
    tag = "Me",
    summary = "Confirm 2FA with a first code",
    request_body = TwoFactorConfirm,
    responses((status = 204, description = "2FA is on."), (status = 422, description = "Wrong code or no pending secret.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V021"])), ("x-services" = json!(["ConfirmTwoFactorAuthentication"])))
)]
pub async fn confirm_two_factor(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    Json(body): Json<TwoFactorConfirm>,
) -> ApiResult<StatusCode> {
    let secret = two_factor::secret(&state.config, &id.user);
    let code = clean(body.code).unwrap_or_default();
    let now = chrono::Utc::now().timestamp() as u64;
    let ok = secret.is_some_and(|s| two_factor::verify(id.user.id, &s, &id.user.email, &code, now));
    if !ok {
        let msg = i18n::t(&i18n::current(), "The provided two factor authentication code was invalid.", &[]);
        return Err(ApiError::field("code", "invalid", msg));
    }
    let mut u = id.user.into_active_model();
    u.two_factor_confirmed_at = Set(Some(crate::db::now()));
    u.update(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/me/two-factor",
    operation_id = "disableTwoFactor",
    tag = "Me",
    summary = "Turn 2FA off",
    request_body = PasswordConfirmation,
    responses((status = 204, description = "2FA is off."), (status = 403, description = "`password_confirmation_required`.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V022"])), ("x-services" = json!(["DisableTwoFactorAuthentication"])))
)]
pub async fn disable_two_factor(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    let mut u = id.user.into_active_model();
    u.two_factor_secret = Set(None);
    u.two_factor_recovery_codes = Set(None);
    u.two_factor_confirmed_at = Set(None);
    u.update(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/me/two-factor/recovery-codes",
    operation_id = "getTwoFactorRecoveryCodes",
    tag = "Me",
    summary = "The current recovery codes",
    responses((status = 200, description = "The codes.", body = TwoFactorRecoveryCodes), (status = 403, description = "`password_confirmation_required`.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V025"])), ("x-services" = json!([])))
)]
pub async fn get_two_factor_recovery_codes(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
) -> ApiResult<Json<TwoFactorRecoveryCodes>> {
    require_password_confirmation(&id, &session, None)?;
    Ok(Json(TwoFactorRecoveryCodes { recovery_codes: two_factor::recovery_codes(&state.config, &id.user) }))
}

#[utoipa::path(
    post,
    path = "/me/two-factor/recovery-codes",
    operation_id = "regenerateTwoFactorRecoveryCodes",
    tag = "Me",
    summary = "Replace the recovery codes",
    request_body = PasswordConfirmation,
    responses(
        (status = 200, description = "The new codes.", body = TwoFactorRecoveryCodes),
        (status = 409, description = "2FA has no secret."),
    ),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V026"])), ("x-services" = json!(["GenerateNewRecoveryCodes"])))
)]
pub async fn regenerate_two_factor_recovery_codes(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<Json<TwoFactorRecoveryCodes>> {
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    if id.user.two_factor_secret.is_none() {
        return Err(ApiError::conflict("Two-factor authentication is not enabled."));
    }
    let codes = two_factor::new_recovery_codes();
    let mut u = id.user.into_active_model();
    u.two_factor_recovery_codes = Set(Some(two_factor::encrypt_codes(&state.config, &codes)));
    u.update(&state.db).await?;
    Ok(Json(TwoFactorRecoveryCodes { recovery_codes: codes }))
}

#[cfg(test)]
mod tests {
    use super::describe_agent;

    #[test]
    fn describes_common_agents() {
        let mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
        assert_eq!(describe_agent(mac), (Some("OS X".into()), Some("Safari".into()), true));
        let android =
            "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Mobile Safari/537.36";
        assert_eq!(describe_agent(android), (Some("AndroidOS".into()), Some("Chrome".into()), false));
        assert_eq!(describe_agent(""), (None, None, false));
    }
}

// ---------------------------------------------------------------- preferences

#[derive(Deserialize, ToSchema)]
pub struct UserPreferencesUpdate {
    pub name_order: Option<String>,
    /// `last_updated`, `asc` or `desc`.
    pub contact_sort_order: Option<String>,
    pub date_format: Option<String>,
    pub timezone: Option<String>,
    pub number_format: Option<String>,
    /// `km` or `mi`.
    pub distance_format: Option<String>,
    /// `google_maps` or `open_street_maps`.
    pub default_map_site: Option<String>,
    pub locale: Option<String>,
    pub help_shown: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/me/preferences",
    operation_id = "updateMyPreferences",
    tag = "Me",
    summary = "Change display and format preferences",
    request_body = UserPreferencesUpdate,
    responses((status = 200, description = "The preferences.", body = UserPreferences), (status = 422, description = "Validation failed.")),
    extensions(
        ("x-permission" = json!("user")),
        ("x-inventory" = json!(["R038", "R213", "R214", "R215", "R216", "R217", "R218", "R219", "R220"])),
        ("x-services" = json!(["StoreNameOrderPreference", "StoreDateFormatPreference", "StoreTimezone", "StoreNumberFormatPreference", "StoreDistanceFormatPreference", "StoreMapsPreference", "StoreLocale", "StoreHelpPreference", "UpdateContactSortOrder"]))
    )
)]
pub async fn update_my_preferences(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    Json(body): Json<UserPreferencesUpdate>,
) -> ApiResult<Json<UserPreferences>> {
    use crate::domains::settings::manage_user_preferences as prefs;
    let (account_id, author_id) = (id.user.account_id, id.user.id);
    let mut v = Validator::new();
    v.one_of("contact_sort_order", body.contact_sort_order.as_deref(), &["last_updated", "asc", "desc"]);
    v.one_of("date_format", body.date_format.as_deref(), &["MMM DD, YYYY", "DD MMM YYYY", "YYYY/MM/DD", "DD/MM/YYYY"]);
    v.one_of(
        "number_format",
        body.number_format.as_deref(),
        &["locale", "1,234.56", "1 234,56", "1.234,56", "1234.56"],
    );
    v.one_of("distance_format", body.distance_format.as_deref(), &["km", "mi"]);
    v.one_of("default_map_site", body.default_map_site.as_deref(), &["google_maps", "open_street_maps"]);
    if let Some(l) = body.locale.as_deref()
        && !i18n::is_supported(l)
    {
        v.fail("locale", "in", &[]);
    }
    if let Some(tz) = body.timezone.as_deref()
        && tz.parse::<chrono_tz::Tz>().is_err()
    {
        v.fail("timezone", "timezone", &[]);
    }
    v.finish()?;

    let tx = state.db.begin().await?;
    let mut user = id.user;
    if let Some(name_order) = body.name_order {
        user = prefs::StoreNameOrderPreference { account_id, author_id, name_order }.execute(&tx).await?;
    }
    if let Some(order) = body.contact_sort_order {
        let mut u = user.into_active_model();
        u.contact_sort_order = Set(order);
        user = u.update(&tx).await?;
    }
    if let Some(date_format) = body.date_format {
        user = prefs::StoreDateFormatPreference { account_id, author_id, date_format }.execute(&tx).await?;
    }
    if let Some(timezone) = body.timezone {
        user = prefs::StoreTimezone { account_id, author_id, timezone }.execute(&tx).await?;
    }
    if let Some(number_format) = body.number_format {
        user = prefs::StoreNumberFormatPreference { account_id, author_id, number_format }.execute(&tx).await?;
    }
    if let Some(distance_format) = body.distance_format {
        user = prefs::StoreDistanceFormatPreference { account_id, author_id, distance_format }.execute(&tx).await?;
    }
    if let Some(maps_site) = body.default_map_site {
        user = prefs::StoreMapsPreference { account_id, author_id, maps_site }.execute(&tx).await?;
    }
    if let Some(locale) = body.locale {
        user = prefs::StoreLocale { account_id, author_id, locale }.execute(&tx).await?;
    }
    if let Some(visibility) = body.help_shown {
        user = prefs::StoreHelpPreference { account_id, author_id, visibility }.execute(&tx).await?;
    }
    tx.commit().await?;
    Ok(Json(CurrentUser::from(user).preferences))
}

#[utoipa::path(
    delete,
    path = "/me",
    operation_id = "deleteMe",
    tag = "Me",
    summary = "Delete your own user (when enabled)",
    request_body = PasswordConfirmation,
    responses(
        (status = 204, description = "Deleted; the session ends."),
        (status = 403, description = "`password_confirmation_required`."),
        (status = 404, description = "Account deletion is off on this server."),
        (status = 409, description = "You are the account's last administrator."),
    ),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["V032"])), ("x-services" = json!(["DeleteUser", "DestroyUser"])))
)]
pub async fn delete_me(
    State(state): State<AppState>,
    AuthUnverified(id): AuthUnverified,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    if !state.config.enable_account_deletion {
        return Err(ApiError::not_found());
    }
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    if id.user.is_account_administrator {
        let admins = users::Entity::find()
            .filter(users::Column::AccountId.eq(id.user.account_id))
            .filter(users::Column::IsAccountAdministrator.eq(true))
            .all(&state.db)
            .await?;
        if admins.len() <= 1 {
            return Err(ApiError::conflict("You are the account's last administrator."));
        }
    }
    let tx = state.db.begin().await?;
    crate::domains::settings::manage_users::delete_user(&tx, &id.user).await?;
    tx.commit().await?;
    session.destroy();
    Ok(StatusCode::NO_CONTENT)
}

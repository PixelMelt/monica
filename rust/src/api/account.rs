//! `/account` (the account, its storage and users) and `/invitations`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::auth::PasswordConfirmation;
use super::schemas::CurrentUser;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::settings::cancel_account::CancelAccount;
use crate::domains::settings::manage_storage;
use crate::domains::settings::manage_users::{
    AcceptInvitation, DestroyUser, GiveAdministratorPrivilege, InviteUser, RemoveAdministratorPrivilege,
};
use crate::entities::{accounts, users};
use crate::error::{ApiError, ApiResult};
use crate::http::access;
use crate::http::auth::{Auth, require_password_confirmation};
use crate::http::json::Json;
use crate::http::session::{Identity, Session};
use crate::http::state::AppState;
use crate::validate::{Validator, clean};

#[derive(Serialize, ToSchema)]
pub struct Account {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    /// 0 means unlimited (X011).
    pub storage_limit_in_mb: i32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

async fn account_of(state: &AppState, id: &Identity) -> ApiResult<accounts::Model> {
    accounts::Entity::find_by_id(id.user.account_id).one(&state.db).await?.ok_or_else(ApiError::not_found)
}

#[utoipa::path(
    get,
    path = "/account",
    operation_id = "getAccount",
    tag = "Account",
    summary = "The caller's account",
    responses((status = 200, description = "The account.", body = Account)),
    extensions(("x-permission" = json!("account:member")), ("x-inventory" = json!([])), ("x-services" = json!([])))
)]
pub async fn get_account(State(state): State<AppState>, Auth(id): Auth) -> ApiResult<Json<Account>> {
    let a = account_of(&state, &id).await?;
    Ok(Json(Account {
        id: a.id,
        storage_limit_in_mb: a.storage_limit_in_mb,
        created_at: a.created_at.unwrap_or_else(crate::db::now),
        updated_at: a.updated_at.unwrap_or_else(crate::db::now),
    }))
}

#[utoipa::path(
    delete,
    path = "/account",
    operation_id = "cancelAccount",
    tag = "Account",
    summary = "Delete the account and everything in it",
    request_body = PasswordConfirmation,
    responses((status = 202, description = "Deleted."), (status = 403, description = "Not an administrator, or `password_confirmation_required`.")),
    extensions(("x-permission" = json!("account:admin")), ("x-inventory" = json!(["R326"])), ("x-services" = json!(["CancelAccount"])))
)]
pub async fn cancel_account(
    State(state): State<AppState>,
    Auth(id): Auth,
    session: Session,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    access::administrator(&id)?;
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    let tx = state.db.begin().await?;
    CancelAccount { account_id: id.user.account_id, author_id: id.user.id }.execute(&tx).await?;
    tx.commit().await?;
    session.destroy();
    Ok(StatusCode::ACCEPTED)
}

#[derive(Serialize, ToSchema)]
pub struct StorageTypeUsage {
    /// Number of files of this type.
    pub count: u64,
    /// Bytes.
    pub size: u64,
}

impl From<manage_storage::TypeUsage> for StorageTypeUsage {
    fn from(t: manage_storage::TypeUsage) -> Self {
        StorageTypeUsage { count: t.count, size: t.size }
    }
}

/// Bytes summed over the files of all the account's vaults.
#[derive(Serialize, ToSchema)]
pub struct StorageUsage {
    /// 0 means unlimited.
    pub storage_limit_in_mb: i32,
    pub total_size: u64,
    pub photo: StorageTypeUsage,
    pub document: StorageTypeUsage,
    pub avatar: StorageTypeUsage,
}

#[utoipa::path(
    get,
    path = "/account/storage",
    operation_id = "getAccountStorage",
    tag = "Account",
    summary = "Storage used by the account",
    responses((status = 200, description = "The usage.", body = StorageUsage)),
    extensions(("x-permission" = json!("account:member")), ("x-inventory" = json!(["R324"])), ("x-services" = json!([])))
)]
pub async fn get_account_storage(State(state): State<AppState>, Auth(id): Auth) -> ApiResult<Json<StorageUsage>> {
    let a = account_of(&state, &id).await?;
    let u = manage_storage::usage(&state.db, a.id).await?;
    Ok(Json(StorageUsage {
        storage_limit_in_mb: a.storage_limit_in_mb,
        total_size: u.total_size(),
        photo: u.photo.into(),
        document: u.document.into(),
        avatar: u.avatar.into(),
    }))
}

// ---------------------------------------------------------------- users

#[derive(Serialize, ToSchema)]
pub struct AccountUser {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_account_administrator: bool,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub email_verified_at: Option<Timestamp>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub invitation_accepted_at: Option<Timestamp>,
    /// Invited and not accepted yet.
    pub is_invitation_pending: bool,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

impl From<users::Model> for AccountUser {
    fn from(u: users::Model) -> Self {
        AccountUser {
            is_invitation_pending: u.invitation_code.is_some() && u.invitation_accepted_at.is_none(),
            id: u.id,
            email: u.email,
            first_name: u.first_name,
            last_name: u.last_name,
            is_account_administrator: u.is_account_administrator,
            email_verified_at: u.email_verified_at,
            invitation_accepted_at: u.invitation_accepted_at,
            created_at: u.created_at.unwrap_or_else(crate::db::now),
            updated_at: u.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct AccountUserList {
    pub data: Vec<AccountUser>,
}

async fn account_user(state: &AppState, id: &Identity, user_id: Uuid) -> ApiResult<users::Model> {
    users::Entity::find_by_id(user_id)
        .filter(users::Column::AccountId.eq(id.user.account_id))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

#[utoipa::path(
    get,
    path = "/account/users",
    operation_id = "listAccountUsers",
    tag = "Account",
    summary = "The account's users, invited ones included",
    responses((status = 200, description = "By creation date.", body = AccountUserList)),
    extensions(("x-permission" = json!("account:member")), ("x-inventory" = json!(["R229", "R330"])), ("x-services" = json!([])))
)]
pub async fn list_account_users(State(state): State<AppState>, Auth(id): Auth) -> ApiResult<Json<AccountUserList>> {
    let rows = users::Entity::find()
        .filter(users::Column::AccountId.eq(id.user.account_id))
        .order_by_asc(users::Column::CreatedAt)
        .order_by_asc(users::Column::Email)
        .all(&state.db)
        .await?;
    Ok(Json(AccountUserList { data: rows.into_iter().map(Into::into).collect() }))
}

#[derive(Deserialize, ToSchema)]
pub struct AccountUserInvite {
    pub email: Option<String>,
    #[serde(default)]
    pub is_account_administrator: bool,
}

#[utoipa::path(
    post,
    path = "/account/users",
    operation_id = "inviteAccountUser",
    tag = "Account",
    summary = "Invite someone to the account",
    request_body = AccountUserInvite,
    responses((status = 201, description = "The invited user.", body = AccountUser), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("account:admin")), ("x-inventory" = json!(["R231"])), ("x-services" = json!(["InviteUser"])))
)]
pub async fn invite_account_user(
    State(state): State<AppState>,
    Auth(id): Auth,
    Json(body): Json<AccountUserInvite>,
) -> ApiResult<(StatusCode, Json<AccountUser>)> {
    access::administrator(&id)?;
    let email = clean(body.email);
    let mut v = Validator::new();
    if v.required("email", &email) {
        v.email("email", email.as_deref());
    }
    v.finish()?;
    let user = InviteUser {
        account_id: id.user.account_id,
        author_id: id.user.id,
        email: email.unwrap(),
        is_administrator: body.is_account_administrator,
    }
    .execute(&state.db, &state.mailer, &state.config)
    .await?;
    Ok((StatusCode::CREATED, Json(user.into())))
}

#[utoipa::path(
    get,
    path = "/account/users/{user_id}",
    operation_id = "getAccountUser",
    tag = "Account",
    summary = "A user of the account",
    params(("user_id" = uuid::Uuid, Path)),
    responses((status = 200, description = "The user.", body = AccountUser), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("account:member")), ("x-inventory" = json!(["R331"])), ("x-services" = json!([])))
)]
pub async fn get_account_user(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<AccountUser>> {
    Ok(Json(account_user(&state, &id, user_id).await?.into()))
}

#[derive(Deserialize, ToSchema)]
pub struct AccountUserUpdate {
    pub is_account_administrator: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/account/users/{user_id}",
    operation_id = "updateAccountUser",
    tag = "Account",
    summary = "Make a user an administrator, or not",
    params(("user_id" = uuid::Uuid, Path)),
    request_body = AccountUserUpdate,
    responses(
        (status = 200, description = "The user.", body = AccountUser),
        (status = 409, description = "Removing your own privilege."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("account:admin")), ("x-inventory" = json!(["R233"])), ("x-services" = json!(["GiveAdministratorPrivilege", "RemoveAdministratorPrivilege"])))
)]
pub async fn update_account_user(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(user_id): Path<Uuid>,
    Json(body): Json<AccountUserUpdate>,
) -> ApiResult<Json<AccountUser>> {
    access::administrator(&id)?;
    let user = account_user(&state, &id, user_id).await?;
    let mut v = Validator::new();
    v.required("is_account_administrator", &body.is_account_administrator);
    v.finish()?;
    let (account_id, author_id) = (id.user.account_id, id.user.id);
    let user = match body.is_account_administrator {
        Some(true) if !user.is_account_administrator => {
            GiveAdministratorPrivilege { account_id, author_id, user_id }.execute(&state.db).await?
        }
        Some(false) if user.is_account_administrator => {
            RemoveAdministratorPrivilege { account_id, author_id, user_id }.execute(&state.db).await?
        }
        _ => user,
    };
    Ok(Json(user.into()))
}

#[utoipa::path(
    delete,
    path = "/account/users/{user_id}",
    operation_id = "deleteAccountUser",
    tag = "Account",
    summary = "Delete a user of the account",
    params(("user_id" = uuid::Uuid, Path)),
    request_body = PasswordConfirmation,
    responses((status = 204, description = "Deleted."), (status = 409, description = "Yourself.")),
    extensions(("x-permission" = json!("account:admin")), ("x-inventory" = json!(["R234"])), ("x-services" = json!(["DestroyUser"])))
)]
pub async fn delete_account_user(
    State(state): State<AppState>,
    Auth(id): Auth,
    session: Session,
    Path(user_id): Path<Uuid>,
    Json(body): Json<PasswordConfirmation>,
) -> ApiResult<StatusCode> {
    access::administrator(&id)?;
    account_user(&state, &id, user_id).await?;
    require_password_confirmation(&id, &session, body.password.as_deref())?;
    let tx = state.db.begin().await?;
    DestroyUser { account_id: id.user.account_id, author_id: id.user.id, user_id }.execute(&tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- invitations

#[derive(Serialize, ToSchema)]
pub struct Invitation {
    #[schema(value_type = uuid::Uuid)]
    pub invitation_code: Uuid,
    pub email: String,
}

async fn invited(state: &AppState, code: Uuid) -> ApiResult<users::Model> {
    users::Entity::find()
        .filter(users::Column::InvitationCode.eq(code.to_string()))
        .filter(users::Column::InvitationAcceptedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

#[utoipa::path(
    get,
    path = "/invitations/{invitation_code}",
    operation_id = "getInvitation",
    tag = "Auth",
    summary = "A pending invitation",
    security(()),
    params(("invitation_code" = uuid::Uuid, Path)),
    responses((status = 200, description = "The invitation.", body = Invitation), (status = 404, description = "Unknown or already accepted.")),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["R009"])), ("x-services" = json!([])))
)]
pub async fn get_invitation(State(state): State<AppState>, Path(code): Path<Uuid>) -> ApiResult<Json<Invitation>> {
    let user = invited(&state, code).await?;
    Ok(Json(Invitation { invitation_code: code, email: user.email }))
}

#[derive(Deserialize, ToSchema)]
pub struct InvitationAccept {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
}

#[utoipa::path(
    post,
    path = "/invitations/{invitation_code}/accept",
    operation_id = "acceptInvitation",
    tag = "Auth",
    summary = "Accept an invitation and log in",
    security(()),
    params(("invitation_code" = uuid::Uuid, Path)),
    request_body = InvitationAccept,
    responses(
        (status = 200, description = "The new user, logged in.", body = CurrentUser),
        (status = 404, description = "Unknown or already accepted."),
        (status = 422, description = "Validation failed."),
    ),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["R010"])), ("x-services" = json!(["AcceptInvitation", "CreateUserNotificationChannel"])))
)]
pub async fn accept_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(code): Path<Uuid>,
    Json(body): Json<InvitationAccept>,
) -> ApiResult<Json<CurrentUser>> {
    invited(&state, code).await?;
    let first_name = clean(body.first_name);
    let last_name = clean(body.last_name);
    let mut v = Validator::new();
    v.required("first_name", &first_name);
    v.required("last_name", &last_name);
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
    let user = AcceptInvitation {
        invitation_code: code,
        first_name: first_name.unwrap(),
        last_name: last_name.unwrap(),
        password: body.password.unwrap(),
    }
    .execute(&tx, &state.mailer, &state.config)
    .await?;
    tx.commit().await?;
    session.login(user.id, false);
    Ok(Json(user.into()))
}

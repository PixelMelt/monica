//! `/account`: the account and its users.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::auth::PasswordConfirmation;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::settings::manage_users::{
    CreateUser, DestroyUser, GiveAdministratorPrivilege, RemoveAdministratorPrivilege, normalize_username,
    validate_username,
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
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
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
    let a = accounts::Entity::find_by_id(id.user.account_id).one(&state.db).await?.ok_or_else(ApiError::not_found)?;
    Ok(Json(Account {
        id: a.id,
        created_at: a.created_at.unwrap_or_else(crate::db::now),
        updated_at: a.updated_at.unwrap_or_else(crate::db::now),
    }))
}

// ---------------------------------------------------------------- users

#[derive(Serialize, ToSchema)]
pub struct AccountUser {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_account_administrator: bool,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

impl From<users::Model> for AccountUser {
    fn from(u: users::Model) -> Self {
        AccountUser {
            id: u.id,
            username: u.username,
            first_name: u.first_name,
            last_name: u.last_name,
            is_account_administrator: u.is_account_administrator,
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
    summary = "The account's users",
    responses((status = 200, description = "By creation date.", body = AccountUserList)),
    extensions(("x-permission" = json!("account:member")), ("x-inventory" = json!(["R229", "R330"])), ("x-services" = json!([])))
)]
pub async fn list_account_users(State(state): State<AppState>, Auth(id): Auth) -> ApiResult<Json<AccountUserList>> {
    let rows = users::Entity::find()
        .filter(users::Column::AccountId.eq(id.user.account_id))
        .order_by_asc(users::Column::CreatedAt)
        .order_by_asc(users::Column::Username)
        .all(&state.db)
        .await?;
    Ok(Json(AccountUserList { data: rows.into_iter().map(Into::into).collect() }))
}

#[derive(Deserialize, ToSchema)]
pub struct AccountUserCreate {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    /// The user's first password; they can change it in their profile.
    pub password: Option<String>,
    pub password_confirmation: Option<String>,
    #[serde(default)]
    pub is_account_administrator: bool,
}

#[utoipa::path(
    post,
    path = "/account/users",
    operation_id = "createAccountUser",
    tag = "Account",
    summary = "Add a user, with a username and a first password",
    request_body = AccountUserCreate,
    responses((status = 201, description = "The new user.", body = AccountUser), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("account:admin")), ("x-inventory" = json!(["R231"])), ("x-services" = json!(["CreateUser"])))
)]
pub async fn create_account_user(
    State(state): State<AppState>,
    Auth(id): Auth,
    Json(body): Json<AccountUserCreate>,
) -> ApiResult<(StatusCode, Json<AccountUser>)> {
    access::administrator(&id)?;
    let first_name = clean(body.first_name);
    let last_name = clean(body.last_name);
    let mut v = Validator::new();
    v.required("first_name", &first_name);
    v.required("last_name", &last_name);
    let username = normalize_username(body.username.as_deref().unwrap_or_default());
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
    let user = CreateUser {
        account_id: id.user.account_id,
        author_id: id.user.id,
        first_name: first_name.unwrap(),
        last_name: last_name.unwrap(),
        username,
        password: body.password.unwrap(),
        is_administrator: body.is_account_administrator,
    }
    .execute(&state.db)
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
    responses((status = 204, description = "Deleted."), (status = 409, description = "Yourself."), (status = 422, description = "Wrong password.")),
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

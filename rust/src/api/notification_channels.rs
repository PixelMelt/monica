//! `/me/notification-channels`: where reminders are sent (email only).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::paging::{PageQuery, PaginationMeta, paginate};
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::domains::settings::manage_notification_channels::{
    CreateUserNotificationChannel, DestroyUserNotificationChannel, SendTestEmail, TYPE_EMAIL,
    ToggleUserNotificationChannel, VerifyUserNotificationChannelEmailAddress,
};
use crate::entities::{user_notification_channels, user_notification_sent};
use crate::error::{ApiError, ApiResult};
use crate::http::auth::Auth;
use crate::http::json::Json;
use crate::http::session::Identity;
use crate::http::state::AppState;
use crate::validate::{Validator, clean};

#[derive(Serialize, ToSchema)]
pub struct NotificationChannel {
    #[schema(value_type = i64)]
    pub id: Id,
    /// `email`.
    pub r#type: String,
    pub label: Option<String>,
    pub content: String,
    /// `HH:MM`, in the user's timezone.
    pub preferred_time: Option<String>,
    pub active: bool,
    pub fails: i32,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub verified_at: Option<Timestamp>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
}

impl From<user_notification_channels::Model> for NotificationChannel {
    fn from(c: user_notification_channels::Model) -> Self {
        NotificationChannel {
            id: c.id,
            r#type: c.r#type,
            label: c.label,
            content: c.content,
            preferred_time: c.preferred_time.map(|t| t.format("%H:%M").to_string()),
            active: c.active,
            fails: c.fails,
            verified_at: c.verified_at,
            created_at: c.created_at.unwrap_or_else(crate::db::now),
            updated_at: c.updated_at.unwrap_or_else(crate::db::now),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct NotificationChannelList {
    pub data: Vec<NotificationChannel>,
}

async fn channel(state: &AppState, id: &Identity, channel_id: Id) -> ApiResult<user_notification_channels::Model> {
    user_notification_channels::Entity::find_by_id(channel_id)
        .filter(user_notification_channels::Column::UserId.eq(id.user.id))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

#[utoipa::path(
    get,
    path = "/me/notification-channels",
    operation_id = "listNotificationChannels",
    tag = "Me",
    summary = "The user's notification channels",
    responses((status = 200, description = "The channels, by id.", body = NotificationChannelList)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R221"])), ("x-services" = json!([])))
)]
pub async fn list_notification_channels(
    State(state): State<AppState>,
    Auth(id): Auth,
) -> ApiResult<Json<NotificationChannelList>> {
    let rows = user_notification_channels::Entity::find()
        .filter(user_notification_channels::Column::UserId.eq(id.user.id))
        .filter(user_notification_channels::Column::Type.eq(TYPE_EMAIL))
        .order_by_asc(user_notification_channels::Column::Id)
        .all(&state.db)
        .await?;
    Ok(Json(NotificationChannelList { data: rows.into_iter().map(Into::into).collect() }))
}

#[derive(Deserialize, ToSchema)]
pub struct NotificationChannelCreate {
    pub label: Option<String>,
    pub content: Option<String>,
    /// `HH:MM`.
    pub preferred_time: Option<String>,
}

fn is_hh_mm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && s[..2].parse::<u32>().is_ok_and(|h| h < 24)
        && s[3..].parse::<u32>().is_ok_and(|m| m < 60)
        && b.iter().enumerate().all(|(i, c)| i == 2 || c.is_ascii_digit())
}

#[utoipa::path(
    post,
    path = "/me/notification-channels",
    operation_id = "createNotificationChannel",
    tag = "Me",
    summary = "Add an email channel",
    request_body = NotificationChannelCreate,
    responses((status = 201, description = "The channel.", body = NotificationChannel), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R222"])), ("x-services" = json!(["CreateUserNotificationChannel"])))
)]
pub async fn create_notification_channel(
    State(state): State<AppState>,
    Auth(id): Auth,
    Json(body): Json<NotificationChannelCreate>,
) -> ApiResult<(StatusCode, Json<NotificationChannel>)> {
    let label = clean(body.label);
    let content = clean(body.content);
    let preferred_time = clean(body.preferred_time);
    let mut v = Validator::new();
    v.required("label", &label);
    if v.required("content", &content) {
        v.email("content", content.as_deref());
        v.max_len("content", content.as_deref(), 255);
    }
    if v.required("preferred_time", &preferred_time) && !preferred_time.as_deref().is_some_and(is_hh_mm) {
        v.fail("preferred_time", "date_format", &[("format", "H:i")]);
    }
    v.finish()?;
    let channel = CreateUserNotificationChannel {
        account_id: id.user.account_id,
        author_id: id.user.id,
        label: label.unwrap(),
        r#type: TYPE_EMAIL.into(),
        content: content.unwrap(),
        verify_email: Some(true),
        preferred_time,
    }
    .execute(&state.db, &state.mailer, &state.config)
    .await?;
    Ok((StatusCode::CREATED, Json(channel.into())))
}

#[utoipa::path(
    get,
    path = "/me/notification-channels/{notification_channel_id}",
    operation_id = "getNotificationChannel",
    tag = "Me",
    summary = "A notification channel",
    params(("notification_channel_id" = i64, Path)),
    responses((status = 200, description = "The channel.", body = NotificationChannel), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R228"])), ("x-services" = json!([])))
)]
pub async fn get_notification_channel(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(channel_id): Path<i64>,
) -> ApiResult<Json<NotificationChannel>> {
    Ok(Json(channel(&state, &id, Id(channel_id)).await?.into()))
}

#[derive(Deserialize, ToSchema)]
pub struct NotificationChannelUpdate {
    pub active: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/me/notification-channels/{notification_channel_id}",
    operation_id = "updateNotificationChannel",
    tag = "Me",
    summary = "Turn a channel on or off",
    params(("notification_channel_id" = i64, Path)),
    request_body = NotificationChannelUpdate,
    responses((status = 200, description = "The channel.", body = NotificationChannel), (status = 422, description = "Validation failed.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R226"])), ("x-services" = json!(["ToggleUserNotificationChannel", "ScheduleAllContactRemindersForNotificationChannel"])))
)]
pub async fn update_notification_channel(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(channel_id): Path<i64>,
    Json(body): Json<NotificationChannelUpdate>,
) -> ApiResult<Json<NotificationChannel>> {
    let current = channel(&state, &id, Id(channel_id)).await?;
    let mut v = Validator::new();
    v.required("active", &body.active);
    v.finish()?;
    if body.active == Some(current.active) {
        return Ok(Json(current.into()));
    }
    let tx = state.db.begin().await?;
    let channel = ToggleUserNotificationChannel {
        account_id: id.user.account_id,
        author_id: id.user.id,
        user_notification_channel_id: current.id,
    }
    .execute(&tx)
    .await?;
    tx.commit().await?;
    Ok(Json(channel.into()))
}

#[utoipa::path(
    delete,
    path = "/me/notification-channels/{notification_channel_id}",
    operation_id = "deleteNotificationChannel",
    tag = "Me",
    summary = "Delete a channel",
    params(("notification_channel_id" = i64, Path)),
    responses((status = 204, description = "Deleted."), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R227"])), ("x-services" = json!(["DestroyUserNotificationChannel"])))
)]
pub async fn delete_notification_channel(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(channel_id): Path<i64>,
) -> ApiResult<StatusCode> {
    let c = channel(&state, &id, Id(channel_id)).await?;
    DestroyUserNotificationChannel {
        account_id: id.user.account_id,
        author_id: id.user.id,
        user_notification_channel_id: c.id,
    }
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct NotificationLog {
    #[schema(value_type = i64)]
    pub id: Id,
    #[schema(value_type = String, format = DateTime)]
    pub sent_at: Timestamp,
    pub subject_line: String,
    /// Why sending failed; null when it succeeded.
    pub error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct NotificationLogList {
    pub data: Vec<NotificationLog>,
    pub meta: PaginationMeta,
}

#[utoipa::path(
    get,
    path = "/me/notification-channels/{notification_channel_id}/logs",
    operation_id = "listNotificationChannelLogs",
    tag = "Me",
    summary = "What was sent through a channel",
    params(("notification_channel_id" = i64, Path), PageQuery),
    responses((status = 200, description = "Newest first.", body = NotificationLogList)),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R228"])), ("x-services" = json!([])))
)]
pub async fn list_notification_channel_logs(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(channel_id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> ApiResult<Json<NotificationLogList>> {
    let c = channel(&state, &id, Id(channel_id)).await?;
    let select = user_notification_sent::Entity::find()
        .filter(user_notification_sent::Column::UserNotificationChannelId.eq(c.id))
        .order_by_desc(user_notification_sent::Column::SentAt)
        .order_by_desc(user_notification_sent::Column::Id);
    let (rows, meta) = paginate(&state.db, select, &q, 100).await?;
    let data = rows
        .into_iter()
        .map(|r| NotificationLog { id: r.id, sent_at: r.sent_at, subject_line: r.subject_line, error: r.error })
        .collect();
    Ok(Json(NotificationLogList { data, meta }))
}

#[utoipa::path(
    post,
    path = "/me/notification-channels/{notification_channel_id}/test",
    operation_id = "sendNotificationChannelTest",
    tag = "Me",
    summary = "Send a test email",
    params(("notification_channel_id" = i64, Path)),
    responses((status = 204, description = "Sent."), (status = 404, description = "Not found.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R225"])), ("x-services" = json!(["SendTestEmail"])))
)]
pub async fn send_notification_channel_test(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(channel_id): Path<i64>,
) -> ApiResult<StatusCode> {
    let c = channel(&state, &id, Id(channel_id)).await?;
    SendTestEmail { account_id: id.user.account_id, author_id: id.user.id, user_notification_channel_id: c.id }
        .execute(&state.db, &state.mailer)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct NotificationChannelVerification {
    #[schema(value_type = Option<uuid::Uuid>)]
    pub token: Option<crate::db::uuid::Uuid>,
}

#[utoipa::path(
    post,
    path = "/me/notification-channels/{notification_channel_id}/verify",
    operation_id = "verifyNotificationChannel",
    tag = "Me",
    summary = "Verify a channel with the token from its email",
    params(("notification_channel_id" = i64, Path)),
    request_body = NotificationChannelVerification,
    responses((status = 200, description = "The channel.", body = NotificationChannel), (status = 422, description = "Wrong token.")),
    extensions(("x-permission" = json!("user")), ("x-inventory" = json!(["R224"])), ("x-services" = json!(["VerifyUserNotificationChannelEmailAddress", "ScheduleAllContactRemindersForNotificationChannel"])))
)]
pub async fn verify_notification_channel(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path(channel_id): Path<i64>,
    Json(body): Json<NotificationChannelVerification>,
) -> ApiResult<Json<NotificationChannel>> {
    let c = channel(&state, &id, Id(channel_id)).await?;
    let mut v = Validator::new();
    v.required("token", &body.token);
    v.finish()?;
    let tx = state.db.begin().await?;
    let channel = VerifyUserNotificationChannelEmailAddress {
        account_id: id.user.account_id,
        author_id: id.user.id,
        user_notification_channel_id: c.id,
        uuid: body.token.unwrap(),
    }
    .execute(&tx)
    .await
    .map_err(|mut e| {
        for f in &mut e.errors {
            if f.field == "uuid" {
                f.field = "token".into();
            }
        }
        e
    })?;
    tx.commit().await?;
    Ok(Json(channel.into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn preferred_times() {
        assert!(super::is_hh_mm("09:00"));
        assert!(super::is_hh_mm("23:59"));
        assert!(!super::is_hh_mm("24:00"));
        assert!(!super::is_hh_mm("9:00"));
        assert!(!super::is_hh_mm("09:0a"));
    }
}

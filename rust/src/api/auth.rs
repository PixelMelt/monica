//! `/auth`: sessions, registration, 2FA challenge, email verification,
//! password reset and confirmation (api-design.md §7.3).

use axum::http::StatusCode;

use crate::http::session::Session;

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

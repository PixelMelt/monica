//! API errors: RFC 9457 problem details (api-design.md §6).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Map, Value};

/// The stable `code` of a problem (api-design.md §6 table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    BadRequest,
    Unauthenticated,
    CsrfTokenMismatch,
    EmailNotVerified,
    Forbidden,
    TokenAbilityMissing,
    PasswordConfirmationRequired,
    SignupDisabled,
    NotFound,
    Conflict,
    PayloadTooLarge,
    ValidationFailed,
    StorageLimitExceeded,
    VaultUserLimitReached,
    RateLimited,
    InternalError,
}

impl Code {
    pub fn status(self) -> StatusCode {
        match self {
            Code::BadRequest => StatusCode::BAD_REQUEST,
            Code::Unauthenticated => StatusCode::UNAUTHORIZED,
            Code::CsrfTokenMismatch
            | Code::EmailNotVerified
            | Code::Forbidden
            | Code::TokenAbilityMissing
            | Code::PasswordConfirmationRequired
            | Code::SignupDisabled => StatusCode::FORBIDDEN,
            Code::NotFound => StatusCode::NOT_FOUND,
            Code::Conflict => StatusCode::CONFLICT,
            Code::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Code::ValidationFailed | Code::StorageLimitExceeded | Code::VaultUserLimitReached => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Code::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Code::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn default_title(self) -> &'static str {
        match self {
            Code::BadRequest => "The request is malformed.",
            Code::Unauthenticated => "Unauthenticated.",
            Code::CsrfTokenMismatch => "CSRF token mismatch.",
            Code::EmailNotVerified => "Your email address is not verified.",
            Code::Forbidden => "This action is unauthorized.",
            Code::TokenAbilityMissing => "This token can't perform this action.",
            Code::PasswordConfirmationRequired => "Password confirmation required.",
            Code::SignupDisabled => "Registration is currently disabled",
            Code::NotFound => "Not found.",
            Code::Conflict => "This conflicts with the current state.",
            Code::PayloadTooLarge => "The file is too large.",
            Code::ValidationFailed => "The given data was invalid.",
            Code::StorageLimitExceeded => "The account storage limit has been reached.",
            Code::VaultUserLimitReached => "The vault has reached its maximum number of users.",
            Code::RateLimited => "Too many requests.",
            Code::InternalError => "Server error.",
        }
    }
}

/// One failed validation rule.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    pub code: Code,
    pub title: String,
    pub detail: Option<String>,
    pub errors: Vec<FieldError>,
    pub retry_after: Option<u64>,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(code: Code) -> Self {
        ApiError { code, title: code.default_title().to_owned(), detail: None, errors: Vec::new(), retry_after: None }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn not_found() -> Self {
        ApiError::new(Code::NotFound)
    }

    pub fn forbidden() -> Self {
        ApiError::new(Code::Forbidden)
    }

    pub fn unauthenticated() -> Self {
        ApiError::new(Code::Unauthenticated)
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        ApiError::new(Code::Conflict).with_detail(detail)
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!("internal error: {err}");
        ApiError::new(Code::InternalError)
    }

    /// A 422 with one field error.
    pub fn field(field: &str, code: &str, message: impl Into<String>) -> Self {
        let mut e = Errors::default();
        e.add(field, code, message);
        e.into_error()
    }
}

impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        match err {
            sea_orm::DbErr::RecordNotFound(_) => ApiError::not_found(),
            other => ApiError::internal(other),
        }
    }
}

#[derive(Serialize)]
struct Body<'a> {
    #[serde(rename = "type")]
    ty: &'static str,
    status: u16,
    code: Code,
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
    #[serde(skip_serializing_if = "<[FieldError]>::is_empty")]
    errors: &'a [FieldError],
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.code.status();
        let body = Body {
            ty: "about:blank",
            status: status.as_u16(),
            code: self.code,
            title: &self.title,
            detail: self.detail.as_deref(),
            errors: &self.errors,
        };
        let mut res = (status, serde_json::to_vec(&body).unwrap_or_default()).into_response();
        let headers = res.headers_mut();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
        if let Some(secs) = self.retry_after {
            headers.insert(header::RETRY_AFTER, HeaderValue::from(secs));
        }
        res
    }
}

/// Collects field errors; [`Errors::finish`] turns them into a 422.
#[derive(Debug, Default)]
pub struct Errors {
    errors: Vec<FieldError>,
}

impl Errors {
    pub fn add(&mut self, field: &str, code: &str, message: impl Into<String>) {
        self.errors.push(FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.into(),
            params: Map::new(),
        });
    }

    pub fn add_with(&mut self, field: &str, code: &str, message: impl Into<String>, params: Map<String, Value>) {
        self.errors.push(FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.into(),
            params,
        });
    }

    pub fn has(&self, field: &str) -> bool {
        self.errors.iter().any(|e| e.field == field)
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    fn into_error(self) -> ApiError {
        let mut e = ApiError::new(Code::ValidationFailed);
        e.errors = self.errors;
        e
    }

    /// `Ok(())` if nothing failed, otherwise the 422.
    pub fn finish(self) -> ApiResult<()> {
        if self.errors.is_empty() { Ok(()) } else { Err(self.into_error()) }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.title)?;
        if let Some(d) = &self.detail {
            write!(f, " ({d})")?;
        }
        for e in &self.errors {
            write!(f, "; {}: {}", e.field, e.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

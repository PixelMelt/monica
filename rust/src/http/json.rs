//! `Json<T>`: request bodies with api-design.md §6 errors (a field with the
//! wrong JSON type is a 422 `validation_failed` naming the field; malformed
//! JSON is a 400), and JSON responses.

use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{ApiError, Code};
use crate::i18n;

pub struct Json<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for Json<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_json = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("application/json") || ct.contains("+json"));
        if !is_json {
            return Err(ApiError::new(Code::BadRequest).with_detail("Expected an application/json body."));
        }
        let bytes = Bytes::from_request(req, state).await.map_err(|_| ApiError::new(Code::BadRequest))?;
        let de = &mut serde_json::Deserializer::from_slice(&bytes);
        match serde_path_to_error::deserialize(de) {
            Ok(v) => Ok(Json(v)),
            Err(e) => {
                let path = e.path().to_string();
                let inner = e.into_inner();
                if inner.is_syntax() || inner.is_eof() || path == "." {
                    return Err(ApiError::new(Code::BadRequest).with_detail(inner.to_string()));
                }
                let field = path.trim_start_matches('.').to_owned();
                let message = crate::validate::message(&i18n::current(), "type", &field, &[]);
                Err(ApiError::field(&field, "type", message).with_detail(inner.to_string()))
            }
        }
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

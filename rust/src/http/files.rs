//! Uploads and file serving (api-design.md §10, X020): handlers receive a
//! multipart `file` part with [`receive`], store it with
//! `state.storage.put`, and write the `files` row with the domain's
//! service; [`serve`] sends the bytes of a `files` row.

use axum::body::Body;
use axum::extract::Multipart;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::state::AppState;
use crate::domains::settings::manage_storage;
use crate::entities::{accounts, files};
use crate::error::{ApiError, ApiResult, Code};
use crate::storage;
use crate::validate::Validator;

/// A received file.
pub struct Upload {
    pub bytes: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

impl Upload {
    pub fn size(&self) -> i32 {
        i32::try_from(self.bytes.len()).unwrap_or(i32::MAX)
    }
}

/// What an upload endpoint accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Accept {
    /// Photos and avatars: JPEG, PNG, GIF or WebP.
    Images,
    /// Documents: anything.
    Any,
}

/// Reads the `file` part of `multipart`: 413 above `MAX_UPLOAD_SIZE`, 422
/// when it is missing, not an accepted image, or over the account's storage
/// limit (`storage_limit_exceeded`).
pub async fn receive(
    state: &AppState,
    account: &accounts::Model,
    mut multipart: Multipart,
    accept: Accept,
) -> ApiResult<Upload> {
    let max = state.config.max_upload_size_mb * 1024 * 1024;
    let mut upload = None;
    while let Some(mut field) = multipart.next_field().await.map_err(bad_multipart)? {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().unwrap_or("file").to_owned();
        let declared = field.content_type().map(str::to_owned);
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(bad_multipart)? {
            if (bytes.len() + chunk.len()) as u64 > max {
                return Err(ApiError::new(Code::PayloadTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        let mime_type = storage::media_type(&bytes, &name, declared.as_deref());
        upload = Some(Upload { bytes, name, mime_type });
    }

    let mut v = Validator::new();
    let Some(upload) = upload.filter(|u| !u.bytes.is_empty()) else {
        v.fail("file", "required", &[]);
        v.finish()?;
        unreachable!("a failed validation returns");
    };
    if accept == Accept::Images && !storage::is_image(&upload.bytes) {
        v.fail("file", "mimes", &[("values", "jpeg, png, gif, webp")]);
        v.finish()?;
    }

    if account.storage_limit_in_mb != 0 {
        let used = manage_storage::usage(&state.db, account.id).await?.total_size();
        let limit = u64::try_from(account.storage_limit_in_mb).unwrap_or(0) * 1024 * 1024;
        if used + upload.bytes.len() as u64 > limit {
            return Err(ApiError::new(Code::StorageLimitExceeded));
        }
    }
    Ok(upload)
}

fn bad_multipart(e: axum::extract::multipart::MultipartError) -> ApiError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::new(Code::PayloadTooLarge)
    } else {
        ApiError::new(Code::BadRequest).with_detail(e.body_text())
    }
}

/// `variant` of `GET …/files/{file_id}/content`.
#[derive(Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    #[default]
    Original,
    Thumbnail,
}

/// Sends the bytes of `file`: the original (with single-range requests) or
/// the 300×300 thumbnail of an image; `ETag` and an immutable cache (a
/// file's bytes never change), `inline` for images and `attachment` for
/// the rest.
pub async fn serve(
    state: &AppState,
    file: &files::Model,
    variant: Variant,
    headers: &HeaderMap,
) -> ApiResult<Response> {
    let etag = format!("\"{}-{}\"", file.uuid, if variant == Variant::Thumbnail { "t" } else { "o" });
    let cache = HeaderValue::from_static("private, max-age=31536000, immutable");
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()).is_some_and(|v| v == etag || v == "*") {
        return Ok((StatusCode::NOT_MODIFIED, [(header::ETAG, etag), (header::CACHE_CONTROL, cache_str(&cache))])
            .into_response());
    }
    let image = storage::IMAGE_TYPES.contains(&file.mime_type.as_str());
    let (bytes, content_type) = if variant == Variant::Thumbnail && image {
        let (bytes, ty) = state.storage.thumbnail(file.vault_id, &file.uuid).await.map_err(missing)?;
        (bytes, ty.to_owned())
    } else {
        let bytes = state.storage.get(file.vault_id, &file.uuid).await.map_err(missing)?;
        (bytes, file.mime_type.clone())
    };
    let disposition = format!(
        "{}; filename=\"{}\"; filename*=UTF-8''{}",
        if image { "inline" } else { "attachment" },
        file.name.replace(['"', '\\', '\r', '\n'], "_"),
        percent_encode(&file.name)
    );

    let total = bytes.len();
    let range = if variant == Variant::Original {
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|r| parse_range(r, total))
    } else {
        None
    };
    let mut res = match range {
        Some(Err(())) => {
            let mut res = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            res.headers_mut().insert(header::CONTENT_RANGE, hv(&format!("bytes */{total}")));
            return Ok(res);
        }
        Some(Ok((start, end))) => {
            let mut res = Response::new(Body::from(bytes[start..=end].to_vec()));
            *res.status_mut() = StatusCode::PARTIAL_CONTENT;
            res.headers_mut().insert(header::CONTENT_RANGE, hv(&format!("bytes {start}-{end}/{total}")));
            res
        }
        None => Response::new(Body::from(bytes)),
    };
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, hv(&content_type));
    h.insert(header::ETAG, hv(&etag));
    h.insert(header::CACHE_CONTROL, cache);
    h.insert(header::CONTENT_DISPOSITION, hv(&disposition));
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    Ok(res)
}

fn cache_str(v: &HeaderValue) -> String {
    v.to_str().unwrap_or_default().to_owned()
}

fn hv(s: &str) -> HeaderValue {
    HeaderValue::from_str(s).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn missing(e: std::io::Error) -> ApiError {
    if e.kind() == std::io::ErrorKind::NotFound { ApiError::not_found() } else { ApiError::internal(e) }
}

/// One `bytes=a-b` range: `Some(Ok)` to send, `Some(Err)` for 416, `None`
/// to send everything (absent, multi-range or malformed headers).
fn parse_range(header: &str, total: usize) -> Option<Result<(usize, usize), ()>> {
    let spec = header.strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", suffix) => {
            let n: usize = suffix.parse().ok()?;
            if n == 0 {
                return Some(Err(()));
            }
            (total.saturating_sub(n), total.checked_sub(1)?)
        }
        (s, "") => (s.parse().ok()?, total.checked_sub(1)?),
        (s, e) => (s.parse().ok()?, e.parse::<usize>().ok()?.min(total.checked_sub(1)?)),
    };
    if start >= total || start > end {
        return Some(Err(()));
    }
    Some(Ok((start, end)))
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ranges() {
        assert_eq!(parse_range("bytes=0-3", 10), Some(Ok((0, 3))));
        assert_eq!(parse_range("bytes=5-", 10), Some(Ok((5, 9))));
        assert_eq!(parse_range("bytes=-4", 10), Some(Ok((6, 9))));
        assert_eq!(parse_range("bytes=8-100", 10), Some(Ok((8, 9))));
        assert_eq!(parse_range("bytes=10-12", 10), Some(Err(())));
        assert_eq!(parse_range("bytes=0-1,4-5", 10), None);
        assert_eq!(parse_range("items=0-1", 10), None);
        assert_eq!(percent_encode("a b.pdf"), "a%20b.pdf");
    }
}

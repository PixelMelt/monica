//! Personal access tokens (`personal_access_tokens`, X027): the bearer
//! credential of non-browser clients. A token is shown once as
//! `<id>|<40 random characters>`; the table keeps the SHA-256 of the random
//! part, as Sanctum does. Abilities are `read` and `write`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};

use super::session::{Credential, Identity};
use super::state::AppState;
use crate::crypto;
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::entities::{personal_access_tokens, users};
use crate::error::ApiResult;

pub const TOKENABLE_TYPE: &str = "user";
pub const ABILITIES: [&str; 2] = ["read", "write"];

pub fn abilities_of(row: &personal_access_tokens::Model) -> Vec<String> {
    row.abilities.as_deref().and_then(|a| serde_json::from_str(a).ok()).unwrap_or_default()
}

/// Resolves a bearer token to its user; `None` if it is unknown or expired.
pub async fn authenticate(state: &AppState, raw: &str) -> Option<Identity> {
    let (id, secret) = raw.split_once('|')?;
    let id = Id(id.parse().ok()?);
    let row = personal_access_tokens::Entity::find_by_id(id).one(&state.db).await.ok()??;
    if !crypto::eq(&row.token, &crypto::sha256_hex(secret.as_bytes())) || row.tokenable_type != TOKENABLE_TYPE {
        return None;
    }
    if row.expires_at.is_some_and(|e| e <= crate::db::now()) {
        return None;
    }
    let user = users::Entity::find_by_id(row.tokenable_id).one(&state.db).await.ok()??;
    let abilities = abilities_of(&row);
    let mut active = row.into_active_model();
    active.last_used_at = Set(Some(crate::db::now()));
    let _ = active.update(&state.db).await;
    Some(Identity { user, credential: Credential::Token { id, abilities } })
}

/// Creates a token; returns the row and the plain text token.
pub async fn create(
    state: &AppState,
    user: Uuid,
    name: &str,
    abilities: &[String],
) -> ApiResult<(personal_access_tokens::Model, String)> {
    let secret = crypto::random_string(40);
    let now = crate::db::now();
    let row = personal_access_tokens::ActiveModel {
        tokenable_type: Set(TOKENABLE_TYPE.to_owned()),
        tokenable_id: Set(user),
        name: Set(name.to_owned()),
        token: Set(crypto::sha256_hex(secret.as_bytes())),
        abilities: Set(Some(serde_json::to_string(abilities).unwrap_or_else(|_| "[]".into()))),
        last_used_at: Set(None),
        expires_at: Set(None),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;
    let plain = format!("{}|{secret}", row.id);
    Ok((row, plain))
}

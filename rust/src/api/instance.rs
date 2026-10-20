//! `GET /instance`: public information about this instance.

use axum::extract::State;
use sea_orm::EntityTrait;
use serde::Serialize;
use utoipa::ToSchema;

use crate::entities::accounts;
use crate::error::ApiResult;
use crate::http::json::Json;
use crate::http::state::AppState;
use crate::i18n;

#[derive(Serialize, ToSchema)]
pub struct Locale {
    /// A supported locale (`localizer.supported_locales`).
    pub id: String,
    /// The language's own name (`auth.lang` in that locale).
    pub name: String,
    pub dir: Dir,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    Ltr,
    Rtl,
}

#[derive(Serialize, ToSchema)]
pub struct Features {
    /// `DELETE /me` is enabled (server flag, off by default, V032).
    pub account_deletion: bool,
}

#[derive(Serialize, ToSchema)]
pub struct Instance {
    /// False when `APP_DISABLE_SIGNUP` is set and at least one account
    /// exists; the first user can always sign up (X007).
    pub signup_enabled: bool,
    /// `APP_SHOW_DELETION_NOTICE` (X008).
    pub show_deletion_notice: bool,
    /// The locale of this request (X009).
    pub locale: String,
    pub default_locale: String,
    /// Supported locales, by name.
    pub locales: Vec<Locale>,
    pub version: String,
    /// Commit hash of the build, when known.
    pub commit: Option<String>,
    pub features: Features,
}

/// `htmldir()` in `app/Helpers/helpers.php`.
pub fn dir(locale: &str) -> Dir {
    match locale.split('_').next().unwrap_or(locale) {
        "ar" | "arc" | "dv" | "fa" | "ha" | "he" | "khw" | "ks" | "ku" | "ps" | "ur" | "yi" => Dir::Rtl,
        _ => Dir::Ltr,
    }
}

/// `SignupHelper::isEnabled`.
pub async fn signup_enabled(state: &AppState) -> ApiResult<bool> {
    Ok(!state.config.disable_signup || accounts::Entity::find().one(&state.db).await?.is_none())
}

#[utoipa::path(
    get,
    path = "/instance",
    operation_id = "getInstance",
    tag = "Auth",
    summary = "Public information about this instance",
    security(()),
    responses((status = 200, description = "The instance information.", body = Instance)),
    extensions(("x-permission" = json!("public")), ("x-inventory" = json!(["V001", "V008"])), ("x-services" = json!([])))
)]
pub async fn get_instance(State(state): State<AppState>) -> ApiResult<Json<Instance>> {
    let mut locales: Vec<Locale> = i18n::locales()
        .map(|l| Locale {
            id: l.to_owned(),
            name: i18n::group(l, "auth.lang", &[]).unwrap_or_else(|| l.to_owned()),
            dir: dir(l),
        })
        .collect();
    locales.sort_by_key(|l| l.name.to_lowercase());
    Ok(Json(Instance {
        signup_enabled: signup_enabled(&state).await?,
        show_deletion_notice: state.config.show_deletion_notice,
        locale: i18n::current(),
        default_locale: state.config.locale.clone(),
        locales,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: option_env!("MONICA_COMMIT").map(str::to_owned),
        features: Features { account_deletion: state.config.enable_account_deletion },
    }))
}

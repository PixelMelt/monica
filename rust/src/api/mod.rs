//! The `/api/v1` operations and the OpenAPI document generated from them.

pub mod auth;
pub mod instance;
pub mod me;
pub mod schemas;
pub mod vaults;

use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::http::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Monica API",
        version = "1.0.0",
        description = "The single HTTP API of Monica, used by the web SPA and other clients.",
        license(name = "AGPL-3.0-or-later", identifier = "AGPL-3.0-or-later")
    ),
    servers((url = "/api/v1")),
    security(("sessionCookie" = []), ("bearerToken" = [])),
    modifiers(&Security),
    tags(
        (name = "Auth"), (name = "Me"), (name = "Account"), (name = "Vaults"), (name = "Contacts"),
        (name = "Contact modules"), (name = "Groups"), (name = "Journals"), (name = "Files"), (name = "Reference")
    )
)]
struct ApiDoc;

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "sessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                crate::http::session::SESSION_COOKIE,
                "Browser session. Unsafe methods also need the `X-XSRF-TOKEN` header equal to the `XSRF-TOKEN` cookie.",
            ))),
        );
        components.add_security_scheme(
            "bearerToken",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some("Personal access token with abilities `read` and/or `write`."))
                    .build(),
            ),
        );
    }
}

/// Every operation, with its OpenAPI description.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(instance::get_instance))
        .routes(routes!(auth::get_csrf_cookie))
        .routes(routes!(auth::log_in))
        .routes(routes!(auth::complete_two_factor_challenge))
        .routes(routes!(auth::log_out))
        .routes(routes!(auth::verify_email))
        .routes(routes!(auth::send_email_verification))
        .routes(routes!(auth::request_password_reset))
        .routes(routes!(auth::reset_password))
        .routes(routes!(auth::confirm_password))
        .routes(routes!(auth::get_password_confirmation_status))
        .routes(routes!(me::get_me, me::update_me))
        .routes(routes!(me::update_my_password))
        .routes(routes!(me::get_bootstrap))
        .routes(routes!(me::get_my_preferences))
        .routes(routes!(me::list_my_sessions))
        .routes(routes!(me::log_out_other_sessions))
        .routes(routes!(me::list_my_tokens, me::create_my_token))
        .routes(routes!(me::update_my_token, me::delete_my_token))
        .routes(routes!(me::enable_two_factor, me::disable_two_factor))
        .routes(routes!(me::confirm_two_factor))
        .routes(routes!(me::get_two_factor_recovery_codes, me::regenerate_two_factor_recovery_codes))
        .routes(routes!(vaults::list_vaults, vaults::create_vault))
        .routes(routes!(vaults::get_vault, vaults::update_vault, vaults::delete_vault))
        .routes(routes!(vaults::list_vault_members, vaults::add_vault_member))
        .routes(routes!(vaults::list_vault_member_candidates))
        .routes(routes!(vaults::update_vault_member, vaults::remove_vault_member))
}

/// The OpenAPI document of the API.
pub fn openapi() -> utoipa::openapi::OpenApi {
    router().into_openapi()
}

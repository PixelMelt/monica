//! The `/api/v1` operations and the OpenAPI document generated from them.

pub mod auth;
pub mod instance;

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
}

/// The OpenAPI document of the API.
pub fn openapi() -> utoipa::openapi::OpenApi {
    router().into_openapi()
}

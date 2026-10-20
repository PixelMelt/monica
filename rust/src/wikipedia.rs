//! `WikipediaHelper::getInformation` (I004): the summary and thumbnail of a
//! city or country for the address reports. Decision Q1: the server asks
//! Wikipedia when the report is requested, and answers empty fields when
//! that fails or is disabled (`WIKIPEDIA_URL=`).

use std::time::Duration;

use serde::Serialize;
use utoipa::ToSchema;

/// What the report pages show about a place.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct WikipediaInfo {
    pub url: Option<String>,
    pub description: Option<String>,
    pub thumbnail: Option<String>,
}

/// Looks `topic` up in the Wikipedia of `lang` (`currentLang()`).
pub async fn get_information(api_url: Option<&str>, lang: &str, topic: &str) -> WikipediaInfo {
    let Some(api_url) = api_url else { return WikipediaInfo::default() };
    let url = api_url.replace("{lang}", lang);
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(5)).user_agent("Monica").build() {
        Ok(c) => c,
        Err(_) => return WikipediaInfo::default(),
    };
    let query = [
        ("action", "query"),
        ("prop", "description|pageimages"),
        ("titles", topic),
        ("pithumbsize", "400"),
        ("format", "json"),
    ];
    let response = match client.get(&url).query(&query).send().await.and_then(|r| r.error_for_status()) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("wikipedia: {e}");
            return WikipediaInfo::default();
        }
    };
    match response.text().await.map(|body| serde_json::from_str::<serde_json::Value>(&body)) {
        Ok(Ok(json)) => from_response(&json, lang, topic),
        _ => WikipediaInfo::default(),
    }
}

/// The fields of a `query.pages` answer: empty when the first page is
/// `missing` (PHP compared it with `=== true`, which the API never sends,
/// so a missing page gives a URL and empty description, as today).
fn from_response(json: &serde_json::Value, lang: &str, topic: &str) -> WikipediaInfo {
    let page = json["query"]["pages"].as_object().and_then(|pages| pages.values().next());
    if page.is_some_and(|p| p["missing"] == serde_json::Value::Bool(true)) {
        return WikipediaInfo::default();
    }
    WikipediaInfo {
        url: Some(format!("https://{lang}.wikipedia.org/wiki/{}", crate::text::slug(topic))),
        description: page.and_then(|p| p["description"].as_str()).map(str::to_owned),
        thumbnail: page.and_then(|p| p["thumbnail"]["source"].as_str()).map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_first_page() {
        let json = json!({"query": {"pages": {"123": {
            "title": "Paris",
            "description": "Capital of France",
            "thumbnail": {"source": "https://upload.wikimedia.org/paris.jpg"}
        }}}});
        assert_eq!(
            from_response(&json, "en", "Paris"),
            WikipediaInfo {
                url: Some("https://en.wikipedia.org/wiki/paris".into()),
                description: Some("Capital of France".into()),
                thumbnail: Some("https://upload.wikimedia.org/paris.jpg".into()),
            }
        );
        let missing = json!({"query": {"pages": {"-1": {"title": "Nowhere", "missing": ""}}}});
        assert_eq!(from_response(&missing, "fr", "Nowhere").description, None);
        assert_eq!(
            from_response(&missing, "fr", "Nowhere").url.as_deref(),
            Some("https://fr.wikipedia.org/wiki/nowhere")
        );
    }

    #[tokio::test]
    async fn fails_quietly() {
        assert_eq!(get_information(None, "en", "Paris").await, WikipediaInfo::default());
        let unreachable = get_information(Some("http://127.0.0.1:1/w/api.php"), "en", "Paris").await;
        assert_eq!(unreachable, WikipediaInfo::default());
    }

    #[tokio::test]
    async fn asks_the_api_of_the_language() {
        use axum::Router;
        use axum::extract::Query;
        use axum::routing::get;
        let app = Router::new().route(
            "/fr/api.php",
            get(|Query(q): Query<std::collections::HashMap<String, String>>| async move {
                axum::Json(json!({"query": {"pages": {"1": {"description": format!("about {}", q["titles"])}}}}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let info = get_information(Some(&format!("http://{addr}/{{lang}}/api.php")), "fr", "Lyon").await;
        assert_eq!(info.description.as_deref(), Some("about Lyon"));
        assert_eq!(info.url.as_deref(), Some("https://fr.wikipedia.org/wiki/lyon"));
    }
}

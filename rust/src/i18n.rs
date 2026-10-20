//! Translations: the 29 locales of `lang/` (D005).
//!
//! The server translates only what it sends to people directly: error
//! titles, validation messages and notifications (api-design.md §8). App
//! strings come from `lang/<locale>.json`; Laravel's `auth` and `validation`
//! arrays from `lang/<locale>/auth.json` and `lang/<locale>/validation.json`.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// (locale, `lang/<locale>.json`, `lang/<locale>/auth.json`,
/// `lang/<locale>/validation.json`).
macro_rules! locale {
    ($l:literal) => {
        (
            $l,
            include_str!(concat!("../../lang/", $l, ".json")),
            include_str!(concat!("../../lang/", $l, "/auth.json")),
            include_str!(concat!("../../lang/", $l, "/validation.json")),
        )
    };
}

/// The supported locales, `en` first.
const SOURCES: &[(&str, &str, &str, &str)] = &[
    locale!("en"),
    locale!("ar"),
    locale!("bn"),
    locale!("ca"),
    locale!("da"),
    locale!("de"),
    locale!("es"),
    locale!("el"),
    locale!("fr"),
    locale!("he"),
    locale!("hi"),
    locale!("it"),
    locale!("ja"),
    locale!("ml"),
    locale!("nl"),
    locale!("nn"),
    locale!("pa"),
    locale!("pl"),
    locale!("pt"),
    locale!("pt_BR"),
    locale!("ro"),
    locale!("ru"),
    locale!("sv"),
    locale!("te"),
    locale!("tr"),
    locale!("ur"),
    locale!("vi"),
    locale!("zh_CN"),
    locale!("zh_TW"),
];

tokio::task_local! {
    /// The locale of the request being handled (set by the session layer).
    pub static LOCALE: String;
}

/// The current request's locale, or `en` outside a request.
pub fn current() -> String {
    LOCALE.try_with(Clone::clone).unwrap_or_else(|_| "en".to_owned())
}

/// The supported locales (`config/localizer.php`), `en` first.
pub fn locales() -> impl Iterator<Item = &'static str> {
    SOURCES.iter().map(|(l, _, _, _)| *l)
}

pub fn is_supported(locale: &str) -> bool {
    locales().any(|l| l == locale)
}

struct Bundle {
    strings: HashMap<String, String>,
    groups: Value,
}

fn bundles() -> &'static HashMap<&'static str, Bundle> {
    static B: OnceLock<HashMap<&'static str, Bundle>> = OnceLock::new();
    B.get_or_init(|| {
        SOURCES
            .iter()
            .map(|(l, json, auth, validation)| {
                let strings = serde_json::from_str(json).unwrap_or_default();
                let parse = |s: &str| serde_json::from_str(s).unwrap_or(Value::Null);
                let groups = serde_json::json!({ "auth": parse(auth), "validation": parse(validation) });
                (*l, Bundle { strings, groups })
            })
            .collect()
    })
}

/// Laravel's placeholder replacement: `:name`, `:Name` (first letter upper
/// case) and `:NAME` (all upper case), longest names first.
pub fn replace(text: &str, params: &[(&str, &str)]) -> String {
    let mut params: Vec<&(&str, &str)> = params.iter().collect();
    params.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let mut out = text.to_owned();
    for (k, v) in params {
        let mut upper_first = k.chars();
        let ucfirst_key = match upper_first.next() {
            Some(c) => c.to_uppercase().collect::<String>() + upper_first.as_str(),
            None => String::new(),
        };
        let mut vchars = v.chars();
        let ucfirst_val = match vchars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + vchars.as_str(),
            None => String::new(),
        };
        out = out
            .replace(&format!(":{}", k.to_uppercase()), &v.to_uppercase())
            .replace(&format!(":{ucfirst_key}"), &ucfirst_val)
            .replace(&format!(":{k}"), v);
    }
    out
}

/// `__($key, $params)`: the app string `key` in `locale`, falling back to
/// English, then to the key itself.
pub fn t(locale: &str, key: &str, params: &[(&str, &str)]) -> String {
    let b = bundles();
    let text = b
        .get(locale)
        .and_then(|x| x.strings.get(key))
        .or_else(|| b.get("en").and_then(|x| x.strings.get(key)))
        .map(String::as_str)
        .unwrap_or(key);
    replace(text, params)
}

/// `trans('group.path')` for the `validation` and `auth` arrays (dot path,
/// e.g. `validation.max.string`). `None` if missing in
/// both `locale` and English.
pub fn group(locale: &str, path: &str, params: &[(&str, &str)]) -> Option<String> {
    let b = bundles();
    let lookup = |l: &str| {
        let mut v = &b.get(l)?.groups;
        for part in path.split('.') {
            v = v.get(part)?;
        }
        v.as_str().map(str::to_owned)
    };
    lookup(locale).or_else(|| lookup("en")).map(|text| replace(&text, params))
}

/// Picks a supported locale from an `Accept-Language` header, else `en`.
pub fn negotiate(accept_language: Option<&str>) -> &'static str {
    let Some(header) = accept_language else {
        return "en";
    };
    let mut ranges: Vec<(f32, String)> = header
        .split(',')
        .filter_map(|part| {
            let mut it = part.trim().split(';');
            let tag = it.next()?.trim().replace('-', "_");
            let q = it.find_map(|p| p.trim().strip_prefix("q=").and_then(|q| q.parse::<f32>().ok())).unwrap_or(1.0);
            (!tag.is_empty() && q > 0.0).then_some((q, tag))
        })
        .collect();
    ranges.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_, tag) in &ranges {
        if let Some(l) = locales().find(|l| l.eq_ignore_ascii_case(tag)) {
            return l;
        }
        let primary = tag.split('_').next().unwrap_or_default();
        if let Some(l) = locales().find(|l| l.eq_ignore_ascii_case(primary)) {
            return l;
        }
    }
    "en"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_has_both_sources() {
        assert_eq!(locales().count(), 29);
        for l in locales() {
            assert!(!bundles()[l].strings.is_empty(), "{l} strings");
            assert!(group(l, "validation.required", &[]).is_some(), "{l} validation");
        }
    }

    #[test]
    fn replaces_placeholders_like_laravel() {
        assert_eq!(replace(":Name and :name and :NAME", &[("name", "jo")]), "Jo and jo and JO");
        assert_eq!(
            group("en", "validation.max.string", &[("attribute", "title"), ("max", "255")]).unwrap(),
            "The title field must not be greater than 255 characters."
        );
        assert_eq!(
            t("fr", "Reminder for :name", &[("name", "Ann")]),
            t("fr", "Reminder for :name", &[("name", "Ann")])
        );
    }

    #[test]
    fn negotiates_accept_language() {
        assert_eq!(negotiate(Some("fr-CA,fr;q=0.9,en;q=0.8")), "fr");
        assert_eq!(negotiate(Some("pt-BR")), "pt_BR");
        assert_eq!(negotiate(Some("xx, de;q=0.5")), "de");
        assert_eq!(negotiate(None), "en");
    }
}

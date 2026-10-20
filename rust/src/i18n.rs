//! Translations: the 29 locales of `lang/` (D005).
//!
//! The server translates only what it sends to people directly: error
//! titles, validation messages and emails (api-design.md §8). App strings come
//! from `lang/<locale>.json`; Laravel's `validation`, `passwords` and `auth`
//! arrays come from `rust/locales/<locale>.json` (`php rust/locales/generate.php`).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// (locale, `lang/<locale>.json`, `locales/<locale>.json`), `en` first.
const SOURCES: &[(&str, &str, &str)] = &[
    (
        "en",
        include_str!("../../lang/en.json"),
        include_str!("../locales/en.json"),
    ),
    (
        "ar",
        include_str!("../../lang/ar.json"),
        include_str!("../locales/ar.json"),
    ),
    (
        "bn",
        include_str!("../../lang/bn.json"),
        include_str!("../locales/bn.json"),
    ),
    (
        "ca",
        include_str!("../../lang/ca.json"),
        include_str!("../locales/ca.json"),
    ),
    (
        "da",
        include_str!("../../lang/da.json"),
        include_str!("../locales/da.json"),
    ),
    (
        "de",
        include_str!("../../lang/de.json"),
        include_str!("../locales/de.json"),
    ),
    (
        "es",
        include_str!("../../lang/es.json"),
        include_str!("../locales/es.json"),
    ),
    (
        "el",
        include_str!("../../lang/el.json"),
        include_str!("../locales/el.json"),
    ),
    (
        "fr",
        include_str!("../../lang/fr.json"),
        include_str!("../locales/fr.json"),
    ),
    (
        "he",
        include_str!("../../lang/he.json"),
        include_str!("../locales/he.json"),
    ),
    (
        "hi",
        include_str!("../../lang/hi.json"),
        include_str!("../locales/hi.json"),
    ),
    (
        "it",
        include_str!("../../lang/it.json"),
        include_str!("../locales/it.json"),
    ),
    (
        "ja",
        include_str!("../../lang/ja.json"),
        include_str!("../locales/ja.json"),
    ),
    (
        "ml",
        include_str!("../../lang/ml.json"),
        include_str!("../locales/ml.json"),
    ),
    (
        "nl",
        include_str!("../../lang/nl.json"),
        include_str!("../locales/nl.json"),
    ),
    (
        "nn",
        include_str!("../../lang/nn.json"),
        include_str!("../locales/nn.json"),
    ),
    (
        "pa",
        include_str!("../../lang/pa.json"),
        include_str!("../locales/pa.json"),
    ),
    (
        "pl",
        include_str!("../../lang/pl.json"),
        include_str!("../locales/pl.json"),
    ),
    (
        "pt",
        include_str!("../../lang/pt.json"),
        include_str!("../locales/pt.json"),
    ),
    (
        "pt_BR",
        include_str!("../../lang/pt_BR.json"),
        include_str!("../locales/pt_BR.json"),
    ),
    (
        "ro",
        include_str!("../../lang/ro.json"),
        include_str!("../locales/ro.json"),
    ),
    (
        "ru",
        include_str!("../../lang/ru.json"),
        include_str!("../locales/ru.json"),
    ),
    (
        "sv",
        include_str!("../../lang/sv.json"),
        include_str!("../locales/sv.json"),
    ),
    (
        "te",
        include_str!("../../lang/te.json"),
        include_str!("../locales/te.json"),
    ),
    (
        "tr",
        include_str!("../../lang/tr.json"),
        include_str!("../locales/tr.json"),
    ),
    (
        "ur",
        include_str!("../../lang/ur.json"),
        include_str!("../locales/ur.json"),
    ),
    (
        "vi",
        include_str!("../../lang/vi.json"),
        include_str!("../locales/vi.json"),
    ),
    (
        "zh_CN",
        include_str!("../../lang/zh_CN.json"),
        include_str!("../locales/zh_CN.json"),
    ),
    (
        "zh_TW",
        include_str!("../../lang/zh_TW.json"),
        include_str!("../locales/zh_TW.json"),
    ),
];

/// The supported locales (`config/localizer.php`), `en` first.
pub fn locales() -> impl Iterator<Item = &'static str> {
    SOURCES.iter().map(|(l, _, _)| *l)
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
            .map(|(l, json, groups)| {
                let strings = serde_json::from_str(json).unwrap_or_default();
                let groups = serde_json::from_str(groups).unwrap_or(Value::Null);
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

/// `trans('group.path')` for the `validation`, `passwords` and `auth`
/// arrays (dot path, e.g. `validation.max.string`). `None` if missing in
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
    lookup(locale)
        .or_else(|| lookup("en"))
        .map(|text| replace(&text, params))
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
            let q = it
                .find_map(|p| {
                    p.trim()
                        .strip_prefix("q=")
                        .and_then(|q| q.parse::<f32>().ok())
                })
                .unwrap_or(1.0);
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
            assert!(
                group(l, "validation.required", &[]).is_some(),
                "{l} validation"
            );
        }
    }

    #[test]
    fn replaces_placeholders_like_laravel() {
        assert_eq!(
            replace(":Name and :name and :NAME", &[("name", "jo")]),
            "Jo and jo and JO"
        );
        assert_eq!(
            group(
                "en",
                "validation.max.string",
                &[("attribute", "title"), ("max", "255")]
            )
            .unwrap(),
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

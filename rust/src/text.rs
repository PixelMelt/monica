//! Text helpers of `app/Helpers` and Laravel's `Str`: locale-aware sorting
//! (`CollectionHelper::sortByCollator`, X017) and `Str::slug`.

use std::cmp::Ordering;

use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};
use icu_locale_core::Locale;

/// `CollectionHelper::getCollator`: the collator of `locale` (an app locale
/// like `pt_BR`). French sorts accents from the end of the word, as
/// `Collator::FRENCH_COLLATION` does (ICU4X has that ordering for `fr-CA`).
pub fn collator(locale: &str) -> CollatorBorrowed<'static> {
    let lang = locale.split(['-', '_']).next().unwrap_or("en").to_lowercase();
    let tag = if lang == "fr" { "fr-CA".to_owned() } else { locale.replace('_', "-") };
    let prefs = tag
        .parse::<Locale>()
        .or_else(|_| lang.parse::<Locale>())
        .map(|l| CollatorPreferences::from(&l))
        .unwrap_or_default();
    Collator::try_new(prefs, CollatorOptions::default())
        .or_else(|_| Collator::try_new(CollatorPreferences::default(), CollatorOptions::default()))
        .expect("the root collation is compiled in")
}

/// `$collection->sortByCollator($key)` in the locale of the request: a stable
/// sort by the string `key` returns.
pub fn sort_by_collator<T>(items: &mut [T], key: impl Fn(&T) -> String) {
    let c = collator(&crate::i18n::current());
    items.sort_by(|a, b| c.compare(&key(a), &key(b)));
}

/// `sortByCollatorDesc`.
pub fn sort_by_collator_desc<T>(items: &mut [T], key: impl Fn(&T) -> String) {
    let c = collator(&crate::i18n::current());
    items.sort_by(|a, b| c.compare(&key(b), &key(a)));
}

/// Compares two strings in the locale of the request.
pub fn compare(a: &str, b: &str) -> Ordering {
    collator(&crate::i18n::current()).compare(a, b)
}

/// `Str::slug($title)`: ASCII transliteration, `@` as `at`, lower case,
/// runs of separators and spaces as one `-`, trimmed.
pub fn slug(title: &str) -> String {
    let ascii = deunicode::deunicode(title);
    let flipped = ascii.replace('_', "-");
    let with_at = flipped.replace('@', "-at-");
    let kept: String =
        with_at.to_lowercase().chars().filter(|c| *c == '-' || c.is_alphanumeric() || c.is_whitespace()).collect();
    let mut out = String::new();
    let mut pending = false;
    for c in kept.chars() {
        if c == '-' || c.is_whitespace() {
            pending = true;
        } else {
            if pending && !out.is_empty() {
                out.push('-');
            }
            pending = false;
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_with_the_locale() {
        let mut names = vec!["zoé", "Émile", "anna", "Ève", "bob"];
        let c = collator("en");
        names.sort_by(|a, b| c.compare(a, b));
        assert_eq!(names, ["anna", "bob", "Émile", "Ève", "zoé"]);
        let mut words = vec!["côte", "coté", "cote", "côté"];
        let fr = collator("fr");
        words.sort_by(|a, b| fr.compare(a, b));
        assert_eq!(words, ["cote", "côte", "coté", "côté"]);
        let mut sv = vec!["ö", "z", "a"];
        let c = collator("sv");
        sv.sort_by(|a, b| c.compare(a, b));
        assert_eq!(sv, ["a", "z", "ö"]);
    }

    #[test]
    fn slugs_like_laravel() {
        assert_eq!(slug("Hello World"), "hello-world");
        assert_eq!(slug("  Café  au lait "), "cafe-au-lait");
        assert_eq!(slug("foo_bar--baz"), "foo-bar-baz");
        assert_eq!(slug("me@home"), "me-at-home");
        assert_eq!(slug("Ünïcödé!?"), "unicode");
    }
}

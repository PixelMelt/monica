//! Request validation with Laravel's rules and messages (X003).
//!
//! Laravel's `TrimStrings` and `ConvertEmptyStringsToNull` middleware trim
//! every input string and turn empty ones into null (passwords excepted);
//! [`clean`] does the same. Messages come from the translated
//! `validation.php` of the request locale.

use serde_json::{Map, Value};

use crate::error::{ApiResult, Errors};
use crate::i18n;

/// Trims `s`; `None` if it is empty (Laravel's input middleware).
pub fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// The attribute name Laravel shows for `field` (`validation.attributes`,
/// else the field with `_` as spaces).
pub fn attribute(locale: &str, field: &str) -> String {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    i18n::group(locale, &format!("validation.attributes.{leaf}"), &[]).unwrap_or_else(|| leaf.replace('_', " "))
}

/// The message for `rule` (e.g. `required`, `max.string`) on `field`.
pub fn message(locale: &str, rule: &str, field: &str, params: &[(&str, &str)]) -> String {
    let attr = attribute(locale, field);
    let mut all: Vec<(&str, &str)> = vec![("attribute", &attr)];
    all.extend_from_slice(params);
    i18n::group(locale, &format!("validation.{rule}"), &all)
        .unwrap_or_else(|| i18n::replace("The :attribute field is invalid.", &all))
}

/// Collects failures for one request.
pub struct Validator {
    errors: Errors,
    locale: String,
}

impl Default for Validator {
    fn default() -> Self {
        Validator { errors: Errors::default(), locale: i18n::current() }
    }
}

impl Validator {
    pub fn new() -> Self {
        Validator::default()
    }

    /// Records a failure of `rule` on `field`. The error `code` is the rule
    /// name without its type suffix (`max.string` → `max`), other dots
    /// become underscores (`password.letters` → `password_letters`).
    pub fn fail(&mut self, field: &str, rule: &str, params: &[(&str, &str)]) {
        let msg = message(&self.locale, rule, field, params);
        let code = match rule.rsplit_once('.') {
            Some((head, "string" | "numeric" | "array" | "file")) => head.replace('.', "_"),
            _ => rule.replace('.', "_"),
        };
        let code = code.as_str();
        let p: Map<String, Value> = params
            .iter()
            .map(|(k, v)| {
                let val = v.parse::<i64>().map(Value::from).unwrap_or_else(|_| Value::from(*v));
                ((*k).to_owned(), val)
            })
            .collect();
        self.errors.add_with(field, code, msg, p);
    }

    pub fn has(&self, field: &str) -> bool {
        self.errors.has(field)
    }

    /// `required`: the value is present (strings already [`clean`]ed).
    pub fn required<T>(&mut self, field: &str, v: &Option<T>) -> bool {
        if v.is_none() {
            self.fail(field, "required", &[]);
            return false;
        }
        true
    }

    /// `max:n` on a string: at most `n` characters.
    pub fn max_len(&mut self, field: &str, v: Option<&str>, max: usize) {
        if let Some(s) = v
            && s.chars().count() > max
        {
            self.fail(field, "max.string", &[("max", &max.to_string())]);
        }
    }

    /// `min:n` on a string: at least `n` characters.
    pub fn min_len(&mut self, field: &str, v: Option<&str>, min: usize) {
        if let Some(s) = v
            && s.chars().count() < min
        {
            self.fail(field, "min.string", &[("min", &min.to_string())]);
        }
    }

    /// `email`: a plausible address (local@domain, no spaces).
    pub fn email(&mut self, field: &str, v: Option<&str>) {
        if let Some(s) = v
            && !is_email(s)
        {
            self.fail(field, "email", &[]);
        }
    }

    /// `in:a,b,c`.
    pub fn one_of(&mut self, field: &str, v: Option<&str>, allowed: &[&str]) {
        if let Some(s) = v
            && !allowed.contains(&s)
        {
            self.fail(field, "in", &[]);
        }
    }

    /// `min:n` / `max:n` on an integer.
    pub fn between(&mut self, field: &str, v: Option<i64>, min: i64, max: i64) {
        if let Some(n) = v {
            if n < min {
                self.fail(field, "min.numeric", &[("min", &min.to_string())]);
            } else if n > max {
                self.fail(field, "max.numeric", &[("max", &max.to_string())]);
            }
        }
    }

    pub fn finish(self) -> ApiResult<()> {
        self.errors.finish()
    }
}

pub fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !s.chars().any(char::is_whitespace)
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && s.chars().count() <= 254
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_follow_laravel() {
        assert_eq!(message("en", "required", "first_name", &[]), "The first name field is required.");
        assert_eq!(
            message("en", "max.string", "title", &[("max", "255")]),
            "The title field must not be greater than 255 characters."
        );
        assert_ne!(message("fr", "required", "email", &[]), message("en", "required", "email", &[]));
    }

    #[test]
    fn clean_trims_and_nulls_empty_strings() {
        assert_eq!(clean(Some("  a ".into())), Some("a".into()));
        assert_eq!(clean(Some("   ".into())), None);
    }

    #[test]
    fn emails() {
        assert!(is_email("a@b.c"));
        assert!(!is_email("a b@c.d"));
        assert!(!is_email("nope"));
    }
}

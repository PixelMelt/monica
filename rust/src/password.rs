//! Password hashing (argon2id) and Laravel's `Password::defaults()` rules
//! (X006, `AppServiceProvider`): in production at least 8 characters with
//! letters, mixed case, numbers and symbols; otherwise at least 4.

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

use crate::validate::Validator;

pub fn hash(plain: &str) -> String {
    Argon2::default().hash_password(plain.as_bytes()).expect("argon2 with default params").to_string()
}

/// Whether `plain` matches `hash` (false for anything that isn't an
/// argon2 hash, e.g. a null password).
pub fn verify(plain: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(plain.as_bytes(), &h).is_ok())
}

/// Applies the password rules to `field`; `confirmation` is the
/// `<field>_confirmation` value when the rule set includes `confirmed`.
pub fn validate(v: &mut Validator, field: &str, plain: &str, production: bool, confirmation: Option<Option<&str>>) {
    let chars = plain.chars().count();
    let min = if production { 8 } else { 4 };
    if chars < min {
        v.fail(field, "min.string", &[("min", &min.to_string())]);
    }
    if production {
        if !plain.chars().any(char::is_alphabetic) {
            v.fail(field, "password.letters", &[]);
        }
        if !(plain.chars().any(char::is_uppercase) && plain.chars().any(char::is_lowercase)) {
            v.fail(field, "password.mixed", &[]);
        }
        if !plain.chars().any(|c| c.is_numeric()) {
            v.fail(field, "password.numbers", &[]);
        }
        if !plain.chars().any(|c| !c.is_alphanumeric() && !c.is_whitespace()) {
            v.fail(field, "password.symbols", &[]);
        }
    }
    if let Some(conf) = confirmation
        && conf != Some(plain)
    {
        v.fail(field, "confirmed", &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_verify() {
        let h = hash("secret");
        assert!(verify("secret", &h));
        assert!(!verify("other", &h));
        assert!(!verify("secret", "not-a-hash"));
    }

    #[test]
    fn production_rules() {
        let mut v = Validator::new();
        validate(&mut v, "password", "abc", true, None);
        let e = v.finish().unwrap_err();
        let codes: Vec<&str> = e.errors.iter().map(|f| f.code.as_str()).collect();
        assert_eq!(codes, ["min", "password_mixed", "password_numbers", "password_symbols"]);

        let mut v = Validator::new();
        validate(&mut v, "password", "Abcdef1!", true, Some(Some("Abcdef1!")));
        assert!(v.finish().is_ok());

        let mut v = Validator::new();
        validate(&mut v, "password", "abcd", false, Some(Some("abce")));
        assert_eq!(v.finish().unwrap_err().errors[0].code, "confirmed");
    }
}

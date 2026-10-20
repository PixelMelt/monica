//! Two-factor authentication (V018–V026), as Fortify does it: a TOTP secret
//! (SHA-1, 6 digits, 30 s, one step of drift) and eight recovery codes
//! `Str::random(10).'-'.Str::random(10)`, both encrypted in `users`
//! (`two_factor_secret`, `two_factor_recovery_codes`); 2FA counts as enabled
//! once `two_factor_confirmed_at` is set. A code can't be used twice.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use qrcode::QrCode;
use qrcode::render::svg;
use totp_rs::{Algorithm, Builder, Secret, Totp};

use crate::config::Config;
use crate::crypto;
use crate::db::uuid::Uuid;
use crate::entities::users;

pub const ISSUER: &str = "Monica";

pub fn new_secret() -> String {
    Secret::generate().to_base32()
}

pub fn new_recovery_codes() -> Vec<String> {
    (0..8).map(|_| new_recovery_code()).collect()
}

pub fn new_recovery_code() -> String {
    format!("{}-{}", crypto::random_string(10), crypto::random_string(10))
}

fn totp(secret: &str, email: &str) -> Option<Totp> {
    Some(
        Builder::new()
            .with_algorithm(Algorithm::SHA1)
            .with_digits(6)
            .with_skew(1)
            .with_step_duration(30)
            .with_secret(Secret::try_from_base32(secret).ok()?)
            .with_account_name(email)
            .with_issuer(Some(ISSUER))
            .build_noncompliant(),
    )
}

/// `otpauth://totp/Monica:<email>?secret=…&issuer=Monica`.
pub fn otpauth_url(secret: &str, email: &str) -> Option<String> {
    totp(secret, email)?.to_url().ok()
}

/// The QR code of the otpauth URL, as Fortify's `twoFactorQrCodeSvg` (192 px).
pub fn qr_code_svg(url: &str) -> Option<String> {
    let code = QrCode::new(url.as_bytes()).ok()?;
    Some(code.render::<svg::Color<'_>>().min_dimensions(192, 192).quiet_zone(true).build())
}

/// The decrypted secret of `user`, if any.
pub fn secret(config: &Config, user: &users::Model) -> Option<String> {
    crypto::decrypt(&config.key, user.two_factor_secret.as_deref()?)
}

/// The decrypted recovery codes of `user`.
pub fn recovery_codes(config: &Config, user: &users::Model) -> Vec<String> {
    user.two_factor_recovery_codes
        .as_deref()
        .and_then(|c| crypto::decrypt(&config.key, c))
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

pub fn encrypt_codes(config: &Config, codes: &[String]) -> String {
    crypto::encrypt(&config.key, &serde_json::to_string(codes).unwrap_or_else(|_| "[]".into()))
}

/// Whether 2FA is on: a secret and a confirmation.
pub fn enabled(user: &users::Model) -> bool {
    user.two_factor_secret.is_some() && user.two_factor_confirmed_at.is_some()
}

fn used_steps() -> &'static Mutex<HashMap<Uuid, u64>> {
    static USED: OnceLock<Mutex<HashMap<Uuid, u64>>> = OnceLock::new();
    USED.get_or_init(Mutex::default)
}

/// Checks `code` against `secret` at `time`; a time step already used by
/// `user` is refused (Fortify caches used codes for their window).
pub fn verify(user: Uuid, secret: &str, email: &str, code: &str, time: u64) -> bool {
    let Some(t) = totp(secret, email) else { return false };
    let Some(step) = t.check(code.trim(), time) else { return false };
    let mut used = used_steps().lock().unwrap();
    if used.get(&user).is_some_and(|s| *s >= step) {
        return false;
    }
    used.insert(user, step);
    true
}

/// The code for `secret` at `time` (tests).
pub fn code_at(secret: &str, time: u64) -> String {
    totp(secret, "test@example.com").map(|t| t.generate(time).to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_verify_once_and_within_one_step() {
        let s = new_secret();
        let user = Uuid::new();
        let t = 1_700_000_000;
        assert!(verify(user, &s, "a@b.c", &code_at(&s, t), t + 25));
        assert!(!verify(user, &s, "a@b.c", &code_at(&s, t), t + 25), "replay refused");
        assert!(!verify(Uuid::new(), &s, "a@b.c", &code_at(&s, t), t + 120), "too old");
        assert!(otpauth_url(&s, "a@b.c").unwrap().starts_with("otpauth://totp/Monica:a%40b.c?"));
        assert!(qr_code_svg("otpauth://totp/x").unwrap().contains("<svg"));
        assert_eq!(new_recovery_codes().len(), 8);
        assert_eq!(new_recovery_code().len(), 21);
    }
}

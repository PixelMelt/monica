//! Random tokens, hashing, signing and encryption with `APP_KEY`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand::RngExt;
use sha2::{Digest, Sha256};

const ALPHANUMERIC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// `len` random characters from `[A-Za-z0-9]` (Laravel's `Str::random`).
pub fn random_string(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len).map(|_| ALPHANUMERIC[rng.random_range(0..ALPHANUMERIC.len())] as char).collect()
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// HMAC-SHA256 of `data` with `key`, hex encoded.
pub fn sign(key: &[u8; 32], data: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("any key length");
    mac.update(data.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Compares two strings in constant time (for their length).
pub fn eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Encrypts `plain` with AES-256-GCM under `key`: base64 of nonce || ciphertext.
pub fn encrypt(key: &[u8; 32], plain: &str) -> String {
    let cipher = Aes256Gcm::new(key.into());
    let nonce_bytes: [u8; 12] = rand::random();
    let ct = cipher.encrypt(&Nonce::from(nonce_bytes), plain.as_bytes()).expect("encryption cannot fail");
    let mut out = nonce_bytes.to_vec();
    out.extend(ct);
    STANDARD.encode(out)
}

/// Reverses [`encrypt`]; `None` if the value was not encrypted with `key`.
pub fn decrypt(key: &[u8; 32], encoded: &str) -> Option<String> {
    let raw = STANDARD.decode(encoded).ok()?;
    if raw.len() < 12 {
        return None;
    }
    let (nonce, ct) = raw.split_at(12);
    let nonce = Nonce::try_from(nonce).ok()?;
    let plain = Aes256Gcm::new(key.into()).decrypt(&nonce, ct).ok()?;
    String::from_utf8(plain).ok()
}

/// URL-safe base64 without padding.
pub fn b64url(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_round_trips_and_rejects_other_keys() {
        let k = [1u8; 32];
        let enc = encrypt(&k, "secret");
        assert_eq!(decrypt(&k, &enc).as_deref(), Some("secret"));
        assert_eq!(decrypt(&[2u8; 32], &enc), None);
        assert_ne!(encrypt(&k, "secret"), enc, "fresh nonce each time");
    }

    #[test]
    fn random_strings_are_alphanumeric() {
        let s = random_string(40);
        assert_eq!(s.len(), 40);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}

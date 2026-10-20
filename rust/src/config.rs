//! Server configuration, from environment variables (and a `.env` file if
//! one exists). Names follow the Laravel app's `.env` where the setting
//! still exists.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};
use base64::Engine as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Production,
    Local,
    Testing,
}

impl Environment {
    /// `APP_ENV`'s value.
    pub fn name(self) -> &'static str {
        match self {
            Environment::Production => "production",
            Environment::Local => "local",
            Environment::Testing => "testing",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Environment,
    /// `APP_URL`, without a trailing slash.
    pub url: String,
    /// `APP_KEY`: 32 bytes, base64 (optionally prefixed `base64:`). Signs
    /// URLs and encrypts 2FA secrets.
    pub key: [u8; 32],
    pub bind: SocketAddr,
    pub database_url: String,
    /// `DATA_DIR`: uploaded files and the default SQLite database.
    pub data_dir: PathBuf,
    /// `SPA_DIR`: the built frontend served at `/`.
    pub spa_dir: Option<PathBuf>,
    /// `APP_LOCALE`.
    pub locale: String,
    /// `MAX_UPLOAD_SIZE`, megabytes per file.
    pub max_upload_size_mb: u64,
    /// `SESSION_LIFETIME`, minutes of inactivity.
    pub session_lifetime_minutes: i64,
    /// `SESSION_SECURE_COOKIE`; defaults to whether `APP_URL` is https.
    pub secure_cookies: bool,
    /// `PUSHOVER_API_URL`: the Pushover API reminders are sent through.
    pub pushover_api_url: String,
    /// `WIKIPEDIA_URL`: the API the city and country reports read summaries
    /// from (`{lang}` is the request's language); empty disables it (Q1).
    pub wikipedia_url: Option<String>,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn flag(name: &str, default: bool) -> anyhow::Result<bool> {
    match var(name).as_deref() {
        None => Ok(default),
        Some("1" | "true" | "TRUE" | "True" | "yes" | "on") => Ok(true),
        Some("0" | "false" | "FALSE" | "False" | "no" | "off") => Ok(false),
        Some(v) => bail!("{name} must be true or false, got {v:?}"),
    }
}

fn number<T: std::str::FromStr>(name: &str, default: T) -> anyhow::Result<T> {
    match var(name) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|_| anyhow!("{name} must be a number, got {v:?}")),
    }
}

/// Decodes an `APP_KEY` value.
pub fn parse_key(raw: &str) -> anyhow::Result<[u8; 32]> {
    let b64 = raw.strip_prefix("base64:").unwrap_or(raw);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).context("APP_KEY is not valid base64")?;
    bytes.try_into().map_err(|_| anyhow!("APP_KEY must decode to 32 bytes"))
}

/// A new random `APP_KEY` value.
pub fn generate_key() -> String {
    let key: [u8; 32] = rand::random();
    format!("base64:{}", base64::engine::general_purpose::STANDARD.encode(key))
}

impl Config {
    /// Reads the configuration from the environment.
    pub fn from_env() -> anyhow::Result<Config> {
        let _ = dotenvy::dotenv();
        let env = match var("APP_ENV").as_deref() {
            None | Some("production") => Environment::Production,
            Some("local" | "development") => Environment::Local,
            Some("testing") => Environment::Testing,
            Some(v) => bail!("APP_ENV must be production, local or testing, got {v:?}"),
        };
        let url = var("APP_URL").unwrap_or_else(|| "http://localhost:8080".into()).trim_end_matches('/').to_owned();
        let key = parse_key(&var("APP_KEY").context("APP_KEY is not set; generate one with `monica key-generate`")?)?;
        let data_dir = PathBuf::from(var("DATA_DIR").unwrap_or_else(|| "data".into()));
        let database_url = var("DATABASE_URL")
            .unwrap_or_else(|| format!("sqlite://{}?mode=rwc", data_dir.join("monica.sqlite").display()));
        Ok(Config {
            env,
            secure_cookies: flag("SESSION_SECURE_COOKIE", url.starts_with("https://"))?,
            url,
            key,
            bind: var("BIND").unwrap_or_else(|| "0.0.0.0:8080".into()).parse().context("BIND must be host:port")?,
            database_url,
            data_dir,
            spa_dir: var("SPA_DIR").map(PathBuf::from),
            locale: var("APP_LOCALE").unwrap_or_else(|| "en".into()),
            max_upload_size_mb: number("MAX_UPLOAD_SIZE", 10)?,
            session_lifetime_minutes: number("SESSION_LIFETIME", 120)?,
            pushover_api_url: var("PUSHOVER_API_URL").unwrap_or_else(|| crate::pushover::API_URL.into()),
            wikipedia_url: match std::env::var("WIKIPEDIA_URL").ok() {
                Some(url) if url.trim().is_empty() => None,
                Some(url) => Some(url),
                None => Some("https://{lang}.wikipedia.org/w/api.php".into()),
            },
        })
    }

    /// A configuration for tests: SQLite or the given URL.
    pub fn for_tests(database_url: &str, data_dir: PathBuf) -> Config {
        Config {
            env: Environment::Testing,
            url: "http://localhost".into(),
            key: [7; 32],
            bind: "127.0.0.1:0".parse().unwrap(),
            database_url: database_url.to_owned(),
            data_dir,
            spa_dir: None,
            locale: "en".into(),
            max_upload_size_mb: 10,
            session_lifetime_minutes: 120,
            secure_cookies: false,
            // Nothing listens on port 1: tests that send start a fake Pushover.
            pushover_api_url: "http://127.0.0.1:1".into(),
            wikipedia_url: None,
        }
    }

    pub fn is_production(&self) -> bool {
        self.env == Environment::Production
    }
}

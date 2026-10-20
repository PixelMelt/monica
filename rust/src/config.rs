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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailTransport {
    /// SMTP; `encryption` is `tls` (STARTTLS), `ssl` (implicit TLS) or none.
    Smtp { host: String, port: u16, username: Option<String>, password: Option<String>, encryption: Option<String> },
    /// Write messages to the log (development).
    Log,
    /// Keep messages in memory (tests read them back).
    Memory,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Environment,
    /// `APP_URL`, without a trailing slash. Used for links in emails.
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
    /// `APP_DISABLE_SIGNUP` (X007).
    pub disable_signup: bool,
    /// `APP_SHOW_DELETION_NOTICE` (X008).
    pub show_deletion_notice: bool,
    /// `DEFAULT_STORAGE_LIMIT`, megabytes per account (X011).
    pub default_storage_limit_mb: i32,
    /// `MAX_UPLOAD_SIZE`, megabytes per file.
    pub max_upload_size_mb: u64,
    /// `ENABLE_ACCOUNT_DELETION` (V032).
    pub enable_account_deletion: bool,
    /// `SESSION_LIFETIME`, minutes of inactivity.
    pub session_lifetime_minutes: i64,
    /// `SESSION_SECURE_COOKIE`; defaults to whether `APP_URL` is https.
    pub secure_cookies: bool,
    pub mail: MailTransport,
    /// `MAIL_FROM_ADDRESS`, `MAIL_FROM_NAME`.
    pub mail_from: (String, String),
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
        let mail = match var("MAIL_MAILER").as_deref() {
            None | Some("smtp") => match var("MAIL_HOST") {
                Some(host) => MailTransport::Smtp {
                    host,
                    port: number("MAIL_PORT", 587)?,
                    username: var("MAIL_USERNAME"),
                    password: var("MAIL_PASSWORD"),
                    encryption: var("MAIL_ENCRYPTION").filter(|e| e != "null"),
                },
                None => MailTransport::Log,
            },
            Some("log") => MailTransport::Log,
            Some("memory" | "array") => MailTransport::Memory,
            Some(v) => bail!("MAIL_MAILER must be smtp, log or memory, got {v:?}"),
        };
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
            disable_signup: flag("APP_DISABLE_SIGNUP", false)?,
            show_deletion_notice: flag("APP_SHOW_DELETION_NOTICE", false)?,
            default_storage_limit_mb: number("DEFAULT_STORAGE_LIMIT", 50)?,
            max_upload_size_mb: number("MAX_UPLOAD_SIZE", 10)?,
            enable_account_deletion: flag("ENABLE_ACCOUNT_DELETION", false)?,
            session_lifetime_minutes: number("SESSION_LIFETIME", 120)?,
            mail,
            mail_from: (
                var("MAIL_FROM_ADDRESS").unwrap_or_else(|| "hello@example.com".into()),
                var("MAIL_FROM_NAME").unwrap_or_else(|| "Monica".into()),
            ),
            wikipedia_url: match std::env::var("WIKIPEDIA_URL").ok() {
                Some(url) if url.trim().is_empty() => None,
                Some(url) => Some(url),
                None => Some("https://{lang}.wikipedia.org/w/api.php".into()),
            },
        })
    }

    /// A configuration for tests: SQLite or the given URL, in-memory mail.
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
            disable_signup: false,
            show_deletion_notice: false,
            default_storage_limit_mb: 50,
            max_upload_size_mb: 10,
            enable_account_deletion: false,
            session_lifetime_minutes: 120,
            secure_cookies: false,
            mail: MailTransport::Memory,
            mail_from: ("hello@example.com".into(), "Monica".into()),
            wikipedia_url: None,
        }
    }

    pub fn is_production(&self) -> bool {
        self.env == Environment::Production
    }
}

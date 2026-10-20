//! Pushover (<https://pushover.net/api>): where reminders are sent. A
//! notification channel holds a Pushover user key and the token of the
//! Pushover application that sends to it.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// The Pushover API (`PUSHOVER_API_URL` replaces it).
pub const API_URL: &str = "https://api.pushover.net/1";

/// What a Pushover channel stores, as JSON in
/// `user_notification_channels.content`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    /// The user (or group) key messages go to.
    pub user_key: String,
    /// The token of the application that sends them.
    pub app_token: String,
}

impl Credentials {
    pub fn from_content(content: &str) -> anyhow::Result<Credentials> {
        Ok(serde_json::from_str(content)?)
    }

    pub fn to_content(&self) -> String {
        serde_json::to_string(self).expect("credentials serialize")
    }
}

/// A notification; without a title, Pushover shows the application's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub title: Option<String>,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Pushover answered with errors: a wrong token or user key, a message
    /// it won't take.
    #[error("Pushover refused: {0}")]
    Rejected(String),
    /// No answer, or not one from Pushover.
    #[error("Pushover could not be reached: {0}")]
    Unreachable(String),
}

#[derive(Clone)]
pub struct Pushover {
    client: reqwest::Client,
    api_url: String,
}

#[derive(Deserialize)]
struct Answer {
    status: i64,
    #[serde(default)]
    errors: Vec<String>,
}

impl Pushover {
    pub fn new(config: &Config) -> Pushover {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!("Monica/", env!("MONICA_VERSION")))
            .build()
            .expect("HTTP client");
        Pushover { client, api_url: config.pushover_api_url.trim_end_matches('/').to_owned() }
    }

    /// `POST /users/validate.json`: whether the application can send to the
    /// user key.
    pub async fn validate(&self, credentials: &Credentials) -> Result<(), Error> {
        self.post("users/validate.json", &[("token", &credentials.app_token), ("user", &credentials.user_key)]).await
    }

    /// `POST /messages.json`.
    pub async fn send(&self, credentials: &Credentials, message: &Message) -> Result<(), Error> {
        let mut form = vec![("token", &credentials.app_token), ("user", &credentials.user_key)];
        if let Some(title) = &message.title {
            form.push(("title", title));
        }
        form.push(("message", &message.message));
        self.post("messages.json", &form).await
    }

    async fn post(&self, path: &str, form: &[(&str, &String)]) -> Result<(), Error> {
        let res = self
            .client
            .post(format!("{}/{path}", self.api_url))
            .form(form)
            .send()
            .await
            .map_err(|e| Error::Unreachable(e.to_string()))?;
        let status = res.status();
        let body = res.bytes().await.map_err(|e| Error::Unreachable(e.to_string()))?;
        // Refusals come with a 4xx status and a JSON body that lists them.
        let answer: Answer =
            serde_json::from_slice(&body).map_err(|_| Error::Unreachable(format!("HTTP {}", status.as_u16())))?;
        if answer.status == 1 {
            Ok(())
        } else if answer.errors.is_empty() {
            Err(Error::Rejected(format!("HTTP {}", status.as_u16())))
        } else {
            Err(Error::Rejected(answer.errors.join(" ")))
        }
    }
}

//! Outgoing email (I001): SMTP, the log, or memory (tests).

use std::sync::{Arc, Mutex};

use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{Config, MailTransport};

/// An email to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

#[derive(Clone)]
enum Transport {
    Smtp(AsyncSmtpTransport<Tokio1Executor>),
    Log,
    Memory(Arc<Mutex<Vec<Email>>>),
}

#[derive(Clone)]
pub struct Mailer {
    transport: Transport,
    from: Mailbox,
}

impl Mailer {
    pub fn new(config: &Config) -> anyhow::Result<Mailer> {
        let from = Mailbox::new(Some(config.mail_from.1.clone()), config.mail_from.0.parse()?);
        let transport = match &config.mail {
            MailTransport::Smtp { host, port, username, password, encryption } => {
                let builder = match encryption.as_deref() {
                    Some("ssl") => AsyncSmtpTransport::<Tokio1Executor>::relay(host)?,
                    Some("tls") => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
                    _ => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
                };
                let mut builder = builder.port(*port);
                if let (Some(u), Some(p)) = (username, password) {
                    builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
                }
                Transport::Smtp(builder.build())
            }
            MailTransport::Log => Transport::Log,
            MailTransport::Memory => Transport::Memory(Arc::default()),
        };
        Ok(Mailer { transport, from })
    }

    /// Whether mail actually leaves the server. `User::sendEmailVerificationNotification`
    /// marks users verified instead of mailing them when SMTP has no credentials;
    /// callers use this to keep that behaviour (E005).
    pub fn smtp_without_credentials(config: &Config) -> bool {
        matches!(&config.mail, MailTransport::Smtp { username: None, .. } | MailTransport::Smtp { password: None, .. })
    }

    pub async fn send(&self, email: Email) -> anyhow::Result<()> {
        match &self.transport {
            Transport::Smtp(smtp) => {
                let msg = Message::builder()
                    .from(self.from.clone())
                    .to(email.to.parse()?)
                    .subject(email.subject)
                    .multipart(MultiPart::alternative_plain_html(email.text, email.html))?;
                smtp.send(msg).await?;
            }
            Transport::Log => {
                tracing::info!(to = %email.to, subject = %email.subject, "mail:\n{}", email.text)
            }
            Transport::Memory(sent) => sent.lock().unwrap().push(email),
        }
        Ok(())
    }

    /// Messages kept by the memory transport (tests).
    pub fn sent(&self) -> Vec<Email> {
        match &self.transport {
            Transport::Memory(sent) => sent.lock().unwrap().clone(),
            _ => Vec::new(),
        }
    }

    /// Text and HTML bodies of a simple notification email: lines, an
    /// optional action link, and closing lines, like Laravel's `MailMessage`.
    pub fn render(lines: &[String], action: Option<(&str, &str)>, outro: &[String]) -> (String, String) {
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
        let mut text = lines.join("\n\n");
        let mut html = lines.iter().map(|l| format!("<p>{}</p>", esc(l))).collect::<String>();
        if let Some((label, url)) = action {
            text.push_str(&format!("\n\n{label}: {url}"));
            html.push_str(&format!("<p><a href=\"{}\">{}</a></p>", esc(url), esc(label)));
        }
        for l in outro {
            text.push_str(&format!("\n\n{l}"));
            html.push_str(&format!("<p>{}</p>", esc(l)));
        }
        (text, format!("<!doctype html><html><body>{html}</body></html>"))
    }
}

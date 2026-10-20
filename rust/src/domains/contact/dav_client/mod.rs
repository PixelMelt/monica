//! `app/Domains/Contact/DavClient`: the CardDAV client that synchronizes a
//! vault with a remote address book (I003), the model helpers of
//! `app/Models/AddressBookSubscription.php`, and the console commands
//! `NewAddressBookSubscription` (K003) and `UpdateAddressBookSubscription`
//! (K007). Queued jobs and batches run synchronously.
//!
//! `Log::channel('database')` goes to `tracing`, as in
//! [`crate::domains::contact::dav`]; `Log::shareContext()` is a span with the
//! `addressbook_subscription_id` field.
//!
//! The client calls the CardDAV backend of the DAV server
//! (`CardDAVBackend`, [`crate::dav::backend`]).

pub mod dav;
pub mod jobs;
pub mod services;
pub mod utils;

use std::io::Write as _;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use self::dav::DavClient;
use self::jobs::SynchronizeAddressBooks;
use self::services::CreateAddressBookSubscription;
use crate::config::Config;
use crate::dav::backend::Ctx;
use crate::dav::server::DavError;
use crate::db::uuid::Uuid;
use crate::domains::contact::dav::php_empty;
use crate::entities::{addressbook_subscriptions, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::http::state::AppState;

/// `AddressBookSubscription::WAY_PUSH`: allow push changes to the distant server.
pub const WAY_PUSH: i64 = 0x1;

/// `AddressBookSubscription::WAY_GET`: allow get changes from the distant server.
pub const WAY_GET: i64 = 0x2;

/// `AddressBookSubscription::WAY_BOTH`: allow both push and get changes.
pub const WAY_BOTH: i64 = WAY_GET | WAY_PUSH;

/// The exceptions the client code throws and catches.
#[derive(Debug)]
pub enum Exception {
    /// `Illuminate\Http\Client\RequestException`: a 4xx or 5xx answer.
    Request(Box<dav::Response>),
    /// `Illuminate\Http\Client\ConnectionException`.
    Connection(String),
    /// `DavClientException`.
    DavClient(String),
    /// `DavServerNotCompliantException` (a `DavClientException`).
    DavServerNotCompliant(String),
    /// Any other exception or error of the client code (unreadable XML, a
    /// PHP `TypeError`).
    Error(String),
    /// An exception of a service or of the database.
    Api(ApiError),
}

impl Exception {
    /// `$e->getMessage()`.
    pub fn message(&self) -> String {
        match self {
            Exception::Request(response) => {
                let message = format!("HTTP request returned status code {}", response.status);
                let summary: String = response.body.chars().take(120).collect();
                if summary.is_empty() {
                    message
                } else if response.body.chars().count() > 120 {
                    format!("{message}:\n{summary} (truncated...)\n")
                } else {
                    format!("{message}:\n{summary}\n")
                }
            }
            Exception::Connection(m) | Exception::DavClient(m) | Exception::DavServerNotCompliant(m) => m.clone(),
            Exception::Error(m) => m.clone(),
            Exception::Api(e) => e
                .errors
                .first()
                .map(|f| f.message.clone())
                .or_else(|| e.detail.clone())
                .unwrap_or_else(|| e.title.clone()),
        }
    }
}

impl From<ApiError> for Exception {
    fn from(err: ApiError) -> Self {
        Exception::Api(err)
    }
}

impl From<sea_orm::DbErr> for Exception {
    fn from(err: sea_orm::DbErr) -> Self {
        Exception::Api(err.into())
    }
}

impl From<DavError> for Exception {
    /// An exception of the DAV server's backend.
    fn from(err: DavError) -> Self {
        Exception::Error(err.message)
    }
}

/// `$subscription->isWayPush`.
fn is_way_push(subscription: &addressbook_subscriptions::Model) -> bool {
    (subscription.sync_way.0 & WAY_PUSH) == WAY_PUSH
}

/// `$subscription->isWayGet`.
fn is_way_get(subscription: &addressbook_subscriptions::Model) -> bool {
    (subscription.sync_way.0 & WAY_GET) == WAY_GET
}

/// `$subscription->password`: `decrypt($value, true)`.
fn password(config: &Config, subscription: &addressbook_subscriptions::Model) -> Result<String, Exception> {
    crate::crypto::decrypt(&config.key, &subscription.password)
        .ok_or_else(|| Exception::Error("The payload is invalid.".to_owned()))
}

/// `$subscription->getClient()`: a new client.
fn get_client(config: &Config, subscription: &addressbook_subscriptions::Model) -> Result<DavClient, Exception> {
    let mut client = DavClient::new(config);
    client.set_base_uri(&subscription.uri).set_credentials(&subscription.username, &password(config, subscription)?);
    Ok(client)
}

/// `HasCapability::hasCapability()`.
fn has_capability(subscription: &addressbook_subscriptions::Model, capability: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(&subscription.capabilities)
        .ok()
        .and_then(|c| c.get(capability).and_then(serde_json::Value::as_bool))
        .unwrap_or(false)
}

/// `$subscription->user`.
async fn user<C: ConnectionTrait>(db: &C, subscription: &addressbook_subscriptions::Model) -> ApiResult<users::Model> {
    users::Entity::find_by_id(subscription.user_id).one(db).await?.ok_or_else(ApiError::not_found)
}

/// `app(CardDAVBackend::class)->withUser($user)`.
fn with_user<'a>(db: &'a DatabaseConnection, config: &'a Config, user: &users::Model) -> Ctx<'a> {
    Ctx { db, config, user: user.clone() }
}

/// `Log::shareContext(['addressbook_subscription_id' => …])`.
fn share_context(subscription: &addressbook_subscriptions::Model) -> tracing::Span {
    tracing::info_span!("addressbook_subscription", addressbook_subscription_id = %subscription.id)
}

// ---- Console commands

/// The options of `monica:newaddressbooksubscription`.
#[derive(Debug, Clone, Default)]
pub struct NewAddressBookSubscriptionOptions {
    /// `--email`: Monica account to add subscription to.
    pub email: Option<String>,
    /// `--vaultId`: Id of the vault to add subscription to.
    pub vault_id: Option<String>,
    /// `--url`: CardDAV url of the address book.
    pub url: Option<String>,
    /// `--login`: Login.
    pub login: Option<String>,
    /// `--password`: Password of the account.
    pub password: Option<String>,
    /// `--pushonly`: Set only push way.
    pub pushonly: bool,
    /// `--getonly`: Set only get way.
    pub getonly: bool,
}

/// `NewAddressBookSubscription::handle()` (K003, `monica:newaddressbooksubscription`):
/// add a new dav subscription. Returns the exit code.
pub async fn new_address_book_subscription(state: &AppState, options: NewAddressBookSubscriptionOptions) -> i32 {
    let db = &state.db;
    let config = &state.config;
    let Some(user) = command_user(db, options.email.as_deref()).await else {
        return 1;
    };
    let Some(vault) = command_vault(db, options.vault_id.as_deref()).await else {
        return 2;
    };

    if user.account_id != vault.account_id {
        eprintln!("Vault does not belong to this account");

        return 3;
    }

    let pushonly = options.pushonly;
    let getonly = options.getonly;
    if pushonly && getonly {
        eprintln!("Cannot set both pushonly and getonly");

        return 4;
    }

    let url = options.url.unwrap_or_else(|| ask("CardDAV url of the address book"));
    let login = options.login.unwrap_or_else(|| ask("Login name"));
    let password = options.password.unwrap_or_else(|| ask("User password"));

    let result = async {
        let subscription = CreateAddressBookSubscription {
            account_id: user.account_id,
            vault_id: vault.id,
            author_id: user.id,
            base_uri: url,
            username: login,
            password,
        }
        .execute(db, config)
        .await?;

        if !pushonly && !getonly {
            return Ok(subscription);
        }
        let mut am: addressbook_subscriptions::ActiveModel = subscription.into();
        am.sync_way = Set(if pushonly { WAY_PUSH } else { WAY_GET }.into());
        Ok::<_, Exception>(am.update(db).await?)
    }
    .await;
    let subscription = match result {
        Ok(subscription) => subscription,
        Err(e) => {
            eprintln!("Could not add subscription");
            eprintln!("{}", e.message());

            return -1;
        }
    };

    println!("Subscription added: {}", subscription.id);
    if let Err(e) = (SynchronizeAddressBooks { subscription, force: true }).handle(db, config).await {
        tracing::error!("SynchronizeAddressBooks: {e}");
    }

    0
}

/// `NewAddressBookSubscription::user()`.
async fn command_user<C: ConnectionTrait>(db: &C, email: Option<&str>) -> Option<users::Model> {
    let Some(email) = email else {
        eprintln!("Please provide an email address");

        return None;
    };

    match users::Entity::find().filter(users::Column::Email.eq(email)).one(db).await {
        Ok(Some(user)) => Some(user),
        _ => {
            eprintln!("Could not find user");

            None
        }
    }
}

/// `NewAddressBookSubscription::vault()`.
async fn command_vault<C: ConnectionTrait>(db: &C, vault_id: Option<&str>) -> Option<vaults::Model> {
    let Some(vault_id) = vault_id else {
        eprintln!("Please provide an vaultId");

        return None;
    };

    let vault = match vault_id.parse::<Uuid>() {
        Ok(id) => vaults::Entity::find_by_id(id).one(db).await.ok().flatten(),
        Err(_) => None,
    };
    if vault.is_none() {
        eprintln!("Could not find vault");
    }
    vault
}

/// `$this->ask($question)` / `$this->secret($question)`: a line of the standard input.
fn ask(question: &str) -> String {
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    answer.trim_end_matches(['\r', '\n']).to_owned()
}

/// `UpdateAddressBookSubscription::handle()` (K007, `monica:updateaddressbooksubscription`):
/// synchronize the subscription `--subscriptionId`, fully with `--force`.
pub async fn update_address_book_subscription(state: &AppState, subscription_id: &str, force: bool) -> ApiResult<()> {
    let id = subscription_id.parse::<Uuid>().map_err(|_| ApiError::not_found())?;
    let subscription =
        addressbook_subscriptions::Entity::find_by_id(id).one(&state.db).await?.ok_or_else(ApiError::not_found)?;

    SynchronizeAddressBooks { subscription, force }.handle(&state.db, &state.config).await
}

/// `UpdateAddressBooks` (J002, hourly): synchronize the subscriptions that are due.
pub async fn update_address_books(state: &AppState) -> anyhow::Result<()> {
    jobs::UpdateAddressBooks.handle(&state.db, &state.config).await?;
    Ok(())
}

#[cfg(test)]
mod tests;

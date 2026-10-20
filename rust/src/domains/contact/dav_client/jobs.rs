//! `app/Domains/Contact/DavClient/Jobs`, and the batch they run in
//! (`Bus::batch()`), both run now.

use std::collections::VecDeque;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use tracing::Instrument;

use super::dav::{NS_CARDDAV, Prop, Value};
use super::services::{SynchronizeAddressBook, UpdateSubscriptionLocalSyncToken};
use super::utils::ContactDto;
use super::{Exception, get_client, php_empty, share_context, user, with_user};
use crate::config::Config;
use crate::dav::backend;
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::contact::dav::jobs::UpdateVCard;
use crate::entities::{addressbook_subscriptions, contacts, job_batches, vaults};
use crate::error::{ApiError, ApiResult};

/// `PushVCard::MODE_MATCH_NONE`.
pub const MODE_MATCH_NONE: i32 = 0;

/// `PushVCard::MODE_MATCH_ETAG`.
pub const MODE_MATCH_ETAG: i32 = 1;

/// `PushVCard::MODE_MATCH_ANY`.
pub const MODE_MATCH_ANY: i32 = 2;

/// A job of a batch.
pub enum Job {
    DeleteLocalVCard(DeleteLocalVCard),
    DeleteMultipleVCard(DeleteMultipleVCard),
    DeleteVCard(DeleteVCard),
    GetMultipleVCard(GetMultipleVCard),
    GetVCard(GetVCard),
    PushVCard(PushVCard),
    UpdateVCard(UpdateVCard),
}

impl Job {
    /// `handle()`: the jobs it adds to its batch (`$this->batch()->add(…)`).
    async fn handle(self, db: &DatabaseConnection, config: &Config) -> Result<Vec<Job>, Exception> {
        match self {
            Job::DeleteLocalVCard(job) => job.handle(db, config).await.map(|()| Vec::new()),
            Job::DeleteMultipleVCard(job) => Ok(job.handle()),
            Job::DeleteVCard(job) => job.handle(config).await.map(|()| Vec::new()),
            Job::GetMultipleVCard(job) => job.handle(db, config).await,
            Job::GetVCard(job) => job.handle(db, config).await,
            Job::PushVCard(job) => job.handle(db, config).await.map(|()| Vec::new()),
            Job::UpdateVCard(job) => Ok(job.execute(db).await.map(|()| Vec::new())?),
        }
    }
}

/// `Bus::batch($jobs)->then($then)->allowFailures()->onQueue('high')->dispatch()`,
/// run now: every job and the jobs they add, in order, a failure not
/// stopping the others; `then` when all of them succeeded. The batch is
/// recorded in `job_batches`; returns its id.
pub(super) async fn dispatch_batch(
    db: &DatabaseConnection,
    config: &Config,
    jobs: Vec<Job>,
    then: Option<UpdateSubscriptionLocalSyncToken>,
) -> Result<String, Exception> {
    let now = || i32::try_from(chrono::Utc::now().timestamp()).unwrap_or(i32::MAX);
    let total = i32::try_from(jobs.len()).unwrap_or(i32::MAX);
    let batch = job_batches::ActiveModel {
        id: Set(Uuid::new().to_string()),
        name: Set(String::new()),
        total_jobs: Set(total),
        pending_jobs: Set(total),
        failed_jobs: Set(0),
        failed_job_ids: Set("[]".to_owned()),
        options: Set(None),
        cancelled_at: Set(None),
        created_at: Set(now()),
        finished_at: Set(None),
    }
    .insert(db)
    .await?;

    let mut queue: VecDeque<Job> = jobs.into();
    let (mut total_jobs, mut pending_jobs, mut failed_jobs) = (total, total, 0);
    let mut succeeded = false;
    while let Some(job) = queue.pop_front() {
        match job.handle(db, config).await {
            Ok(added) => {
                let count = i32::try_from(added.len()).unwrap_or(i32::MAX);
                total_jobs += count;
                pending_jobs += count;
                queue.extend(added);
                pending_jobs -= 1;
                succeeded = pending_jobs == 0;
            }
            Err(e) => {
                tracing::error!(batch = batch.id, "batched job failed: {}", e.message());
                failed_jobs += 1;
            }
        }
    }

    let mut am: job_batches::ActiveModel = batch.clone().into();
    am.total_jobs = Set(total_jobs);
    am.pending_jobs = Set(pending_jobs);
    am.failed_jobs = Set(failed_jobs);
    if succeeded {
        am.finished_at = Set(Some(now()));
    }
    am.update(db).await?;

    if let Some(then) = then
        && succeeded
    {
        then.execute(db, config).await?;
    }

    Ok(batch.id)
}

/// `DeleteLocalVCard`.
pub struct DeleteLocalVCard {
    pub(super) subscription: addressbook_subscriptions::Model,
    pub(super) uri: String,
}

impl DeleteLocalVCard {
    /// Create a new job instance.
    pub fn new(subscription: addressbook_subscriptions::Model, uri: String) -> Self {
        DeleteLocalVCard { subscription, uri }
    }

    /// Send Delete contact.
    pub async fn handle(self, db: &DatabaseConnection, config: &Config) -> Result<(), Exception> {
        let span = share_context(&self.subscription);
        self.run(db, config).instrument(span).await
    }

    /// Run the job.
    async fn run(&self, db: &DatabaseConnection, config: &Config) -> Result<(), Exception> {
        tracing::debug!("Delete local card {}", self.uri);

        let ctx = with_user(db, config, &user(db, &self.subscription).await?);
        backend::delete_card(&ctx, &self.subscription.vault_id.to_string(), &self.uri).await?;

        Ok(())
    }
}

/// `DeleteMultipleVCard`.
pub struct DeleteMultipleVCard {
    pub(super) subscription: addressbook_subscriptions::Model,
    pub(super) hrefs: Vec<String>,
}

impl DeleteMultipleVCard {
    /// Create a new job instance.
    pub fn new(subscription: addressbook_subscriptions::Model, hrefs: Vec<String>) -> Self {
        DeleteMultipleVCard { subscription, hrefs }
    }

    /// The jobs to add to the batch.
    pub fn handle(self) -> Vec<Job> {
        let span = share_context(&self.subscription);
        span.in_scope(|| self.run())
    }

    /// Run the job.
    fn run(&self) -> Vec<Job> {
        let jobs: Vec<Job> = self.hrefs.iter().map(|href| Job::DeleteLocalVCard(self.delete_v_card(href))).collect();

        tracing::info!("Delete {} card(s) from distant server...", jobs.len());

        jobs
    }

    /// Delete the contact.
    fn delete_v_card(&self, href: &str) -> DeleteLocalVCard {
        DeleteLocalVCard::new(self.subscription.clone(), href.to_owned())
    }
}

/// `DeleteVCard`.
pub struct DeleteVCard {
    pub(super) subscription: addressbook_subscriptions::Model,
    pub(super) uri: String,
}

impl DeleteVCard {
    /// Create a new job instance.
    pub fn new(subscription: addressbook_subscriptions::Model, uri: String) -> Self {
        DeleteVCard { subscription, uri }
    }

    /// Send Delete contact.
    pub async fn handle(self, config: &Config) -> Result<(), Exception> {
        let span = share_context(&self.subscription);
        self.run(config).instrument(span).await
    }

    /// Run the job.
    async fn run(&self, config: &Config) -> Result<(), Exception> {
        tracing::debug!("Delete card {}", self.uri);

        get_client(config, &self.subscription)?.request("DELETE", &self.uri, None, &[]).await?;

        Ok(())
    }
}

/// `GetMultipleVCard`.
pub struct GetMultipleVCard {
    pub(super) subscription: addressbook_subscriptions::Model,
    pub(super) hrefs: Vec<String>,
}

impl GetMultipleVCard {
    /// Create a new job instance.
    pub fn new(subscription: addressbook_subscriptions::Model, hrefs: Vec<String>) -> Self {
        GetMultipleVCard { subscription, hrefs }
    }

    /// The jobs to add to the batch.
    pub async fn handle<C: ConnectionTrait>(self, db: &C, config: &Config) -> Result<Vec<Job>, Exception> {
        let span = share_context(&self.subscription);
        self.run(db, config).instrument(span).await
    }

    /// Run the job.
    async fn run<C: ConnectionTrait>(&self, db: &C, config: &Config) -> Result<Vec<Job>, Exception> {
        let data = self.addressbook_multiget(config).await?;

        let mut jobs = Vec::new();
        for (href, contact) in data.responses.iter().filter(|(_, contact)| contact.status == "200") {
            if let Some(job) = self.update_v_card(db, contact, href).await? {
                jobs.push(Job::UpdateVCard(job));
            }
        }

        tracing::info!("Get {} card(s) from distant server...", jobs.len());

        Ok(jobs)
    }

    /// Update the contact.
    async fn update_v_card<C: ConnectionTrait>(
        &self,
        db: &C,
        contact: &super::dav::StatusList,
        href: &str,
    ) -> ApiResult<Option<UpdateVCard>> {
        let card = contact.get("200", &format!("{{{NS_CARDDAV}}}address-data")).and_then(Value::text);

        let Some(card) = card else {
            return Ok(None);
        };

        Ok(Some(UpdateVCard {
            account_id: vault(db, &self.subscription).await?.account_id,
            author_id: self.subscription.user_id,
            vault_id: self.subscription.vault_id,
            uri: href.to_owned(),
            etag: contact.get("200", "{DAV:}getetag").and_then(Value::text).map(str::to_owned),
            card: card.to_owned(),
            external: Some(true),
        }))
    }

    /// Get addressbook data.
    async fn addressbook_multiget(&self, config: &Config) -> Result<super::dav::MultiStatus, Exception> {
        get_client(config, &self.subscription)?
            .addressbook_multiget(&["{DAV:}getetag".into(), self.get_address_data_property()], &self.hrefs)
            .await
    }

    /// Get data for address-data property.
    fn get_address_data_property(&self) -> Prop {
        let capabilities = serde_json::from_str::<serde_json::Value>(&self.subscription.capabilities).ok();
        let address_data_attributes = match capabilities.as_ref().and_then(|c| c.get("addressData")) {
            Some(serde_json::Value::Object(attributes)) => attributes
                .iter()
                .map(|(name, value)| (name.clone(), value.as_str().map(str::to_owned).unwrap_or_default()))
                .collect(),
            _ => vec![("content-type".to_owned(), "text/vcard".to_owned()), ("version".to_owned(), "4.0".to_owned())],
        };

        Prop { name: format!("{{{NS_CARDDAV}}}address-data"), value: None, attributes: address_data_attributes }
    }
}

/// `GetVCard`.
pub struct GetVCard {
    pub(super) subscription: addressbook_subscriptions::Model,
    pub(super) contact: ContactDto,
}

impl GetVCard {
    /// Create a new job instance.
    pub fn new(subscription: addressbook_subscriptions::Model, contact: ContactDto) -> Self {
        GetVCard { subscription, contact }
    }

    /// The jobs to add to the batch.
    pub async fn handle<C: ConnectionTrait>(self, db: &C, config: &Config) -> Result<Vec<Job>, Exception> {
        let span = share_context(&self.subscription);
        self.run(db, config).instrument(span).await
    }

    /// Run the job.
    async fn run<C: ConnectionTrait>(&self, db: &C, config: &Config) -> Result<Vec<Job>, Exception> {
        tracing::debug!("Get card {}", self.contact.uri());

        let response = get_client(config, &self.subscription)?.request("GET", self.contact.uri(), None, &[]).await?;

        let job = self.update_v_card(db, response.body).await?;

        Ok(vec![Job::UpdateVCard(job)])
    }

    async fn update_v_card<C: ConnectionTrait>(&self, db: &C, card: String) -> ApiResult<UpdateVCard> {
        Ok(UpdateVCard {
            account_id: vault(db, &self.subscription).await?.account_id,
            author_id: self.subscription.user_id,
            vault_id: self.subscription.vault_id,
            uri: self.contact.uri().to_owned(),
            etag: self.contact.etag().map(str::to_owned),
            card,
            external: Some(true),
        })
    }
}

/// `PushVCard`.
pub struct PushVCard {
    pub subscription: addressbook_subscriptions::Model,
    pub uri: String,
    pub etag: Option<String>,
    pub card: String,
    pub contact_id: String,
    pub mode: i32,
}

impl PushVCard {
    /// Create a new job instance.
    pub fn new(
        subscription: addressbook_subscriptions::Model,
        uri: String,
        etag: Option<String>,
        card: String,
        contact_id: String,
        mode: i32,
    ) -> Self {
        PushVCard { subscription, uri, etag, card, contact_id, mode }
    }

    /// Push VCard data to the distance server.
    pub async fn handle<C: ConnectionTrait>(self, db: &C, config: &Config) -> Result<(), Exception> {
        let span = share_context(&self.subscription);
        self.run(db, config).instrument(span).await
    }

    /// Run the job.
    async fn run<C: ConnectionTrait>(mut self, db: &C, config: &Config) -> Result<(), Exception> {
        let contact = match self.contact_id.parse::<Uuid>() {
            Ok(id) => {
                contacts::Entity::live()
                    .filter(contacts::Column::VaultId.eq(self.subscription.vault_id))
                    .filter(contacts::Column::Id.eq(id))
                    .one(db)
                    .await?
            }
            Err(_) => None,
        }
        .ok_or_else(ApiError::not_found)?;

        let etag = self.push_distant(config).await?;

        if contact.distant_uri.is_some() {
            let mut am: contacts::ActiveModel = contact.clone().into();

            am.distant_etag.set_ne(if php_empty(&etag) { None } else { Some(etag) });

            if am.is_changed() {
                am.updated_at = Set(contact.updated_at);
                am.update(db).await?;
            }
        }

        Ok(())
    }

    async fn push_distant(&mut self, config: &Config) -> Result<String, Exception> {
        let mut depth = 1;
        loop {
            tracing::debug!("Push card {}", self.uri);

            let response = get_client(config, &self.subscription)?
                .request("PUT", &self.uri, Some(self.card.clone()), &self.headers())
                .await;

            match response {
                Ok(response) => return Ok(response.header("Etag")),
                Err(Exception::Request(response)) if depth > 0 && response.status == 412 => {
                    // If the status is 412 (Precondition Failed), then we retry once with a mode match NONE
                    self.mode = MODE_MATCH_NONE;

                    depth -= 1;
                }
                Err(e) => {
                    if let Exception::Request(response) = &e {
                        tracing::error!(body = response.body, "PushVCard pushDistant: {}", e.message());
                    }

                    return Err(e);
                }
            }
        }
    }

    /// Get the headers for the request.
    fn headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = Vec::new();

        if self.mode == MODE_MATCH_ETAG {
            headers.push(("If-Match", self.etag.clone().unwrap_or_default()));
        } else if self.mode == MODE_MATCH_ANY {
            headers.push(("If-Match", "*".to_owned()));
        }

        headers
    }
}

/// `SynchronizeAddressBooks`.
pub struct SynchronizeAddressBooks {
    pub subscription: addressbook_subscriptions::Model,
    pub force: bool,
}

impl SynchronizeAddressBooks {
    /// Execute the job.
    pub async fn handle(mut self, db: &DatabaseConnection, config: &Config) -> ApiResult<()> {
        let logid = self.subscription.current_logid.map_or(0, |l| l.0);
        let mut am: addressbook_subscriptions::ActiveModel = self.subscription.clone().into();
        am.current_logid = Set(Some(Id(logid + 1)));
        self.subscription = am.update(db).await?;

        let span = share_context(&self.subscription);

        let locale = user(db, &self.subscription).await?.locale;
        crate::i18n::LOCALE.scope(locale, self.synchronize(db, config)).instrument(span).await
    }

    /// Run synchronization.
    async fn synchronize(self, db: &DatabaseConnection, config: &Config) -> ApiResult<()> {
        tracing::info!("Synchronize addressbook '{}'", vault(db, &self.subscription).await?.name);

        let mut am: addressbook_subscriptions::ActiveModel = self.subscription.clone().into();
        let batch_id = async {
            SynchronizeAddressBook {
                account_id: user(db, &self.subscription).await?.account_id,
                addressbook_subscription_id: self.subscription.id,
                force: Some(self.force),
            }
            .execute(db, config)
            .await
        }
        .await;
        match batch_id {
            Ok(batch_id) => am.last_batch = Set(batch_id),
            Err(e) => tracing::error!("SynchronizeAddressBooks synchronize:{}", e.message()),
        }

        am.last_synchronized_at = Set(Some(crate::db::now()));
        am.update(db).await?;

        Ok(())
    }
}

/// `UpdateAddressBooks`.
pub struct UpdateAddressBooks;

impl UpdateAddressBooks {
    /// Update all address book subscriptions.
    pub async fn handle(self, db: &DatabaseConnection, config: &Config) -> ApiResult<()> {
        let now = crate::db::now();

        let mut last_id: Option<Uuid> = None;
        loop {
            let mut query = addressbook_subscriptions::Entity::find()
                .filter(addressbook_subscriptions::Column::Active.eq(true))
                .order_by_asc(addressbook_subscriptions::Column::Id)
                .limit(200);
            if let Some(last_id) = last_id {
                query = query.filter(addressbook_subscriptions::Column::Id.gt(last_id));
            }
            let subscriptions = query.all(db).await?;
            let count = subscriptions.len();
            if count == 0 {
                break;
            }
            last_id = subscriptions.last().map(|s| s.id);

            self.manage_subscriptions(db, config, subscriptions, now).await;

            if count != 200 {
                break;
            }
        }

        Ok(())
    }

    /// Manage the subscriptions.
    pub(super) async fn manage_subscriptions(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        subscriptions: Vec<addressbook_subscriptions::Model>,
        now: Timestamp,
    ) {
        for subscription in subscriptions.into_iter().filter(|s| self.is_time_to_run_sync(s, now)) {
            if let Err(e) = (SynchronizeAddressBooks { subscription, force: false }).handle(db, config).await {
                tracing::error!("SynchronizeAddressBooks: {e}");
            }
        }
    }

    /// Test if the last synchronized timestamp is older than the subscription's frequency time.
    fn is_time_to_run_sync(&self, subscription: &addressbook_subscriptions::Model, now: Timestamp) -> bool {
        match subscription.last_synchronized_at {
            None => true,
            Some(last) => last.0 + chrono::Duration::minutes(i64::from(subscription.frequency)) < now.0,
        }
    }
}

/// `$subscription->vault`.
async fn vault<C: ConnectionTrait>(
    db: &C,
    subscription: &addressbook_subscriptions::Model,
) -> ApiResult<vaults::Model> {
    vaults::Entity::find_by_id(subscription.vault_id).one(db).await?.ok_or_else(ApiError::not_found)
}

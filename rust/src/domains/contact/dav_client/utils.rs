//! `app/Domains/Contact/DavClient/Services/Utils`: `AddressBookGetter`,
//! `AddressBookSynchronizer`, the `PrepareJobs*` classes and the
//! `Model` DTOs.

use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use super::dav::{self, DavClient, MultiStatus, NS_CARDDAV, PropFind, StatusList, Value};
use super::jobs::{
    DeleteLocalVCard, DeleteMultipleVCard, DeleteVCard, GetMultipleVCard, GetVCard, Job, MODE_MATCH_ANY,
    MODE_MATCH_ETAG, MODE_MATCH_NONE, PushVCard, dispatch_batch,
};
use super::services::UpdateSubscriptionLocalSyncToken;
use super::{Exception, get_client, has_capability, is_way_get, is_way_push, php_empty, with_user};
use crate::config::Config;
use crate::dav::backend::{self, DavResource};
use crate::domains::contact::dav::decode_uri;
use crate::entities::{addressbook_subscriptions, user_vault, users};
use crate::error::ApiResult;

/// `ContactDto` and `ContactDeleteDto`.
#[derive(Debug, Clone, PartialEq)]
pub enum ContactDto {
    /// `ContactDto`.
    Contact { uri: String, etag: Option<String> },
    /// `ContactDeleteDto`.
    Delete { uri: String },
}

impl ContactDto {
    /// `$contact->uri`.
    pub fn uri(&self) -> &str {
        match self {
            ContactDto::Contact { uri, .. } | ContactDto::Delete { uri } => uri,
        }
    }

    /// `$contact->etag`.
    pub fn etag(&self) -> Option<&str> {
        match self {
            ContactDto::Contact { etag, .. } => etag.as_deref(),
            ContactDto::Delete { .. } => None,
        }
    }
}

/// `$data['capabilities']` of a subscription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(rename = "addressbookMultiget")]
    pub addressbook_multiget: bool,
    #[serde(rename = "addressbookQuery")]
    pub addressbook_query: bool,
    #[serde(rename = "syncCollection")]
    pub sync_collection: bool,
    #[serde(rename = "addressData")]
    pub address_data: AddressData,
}

/// `$capabilities['addressData']`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddressData {
    #[serde(rename = "content-type")]
    pub content_type: Option<String>,
    pub version: Option<String>,
}

/// What `AddressBookGetter::execute()` returns.
#[derive(Debug, Clone, PartialEq)]
pub struct AddressBookData {
    pub uri: String,
    pub capabilities: Capabilities,
    pub name: Value,
}

/// `AddressBookGetter`.
pub struct AddressBookGetter {
    client: DavClient,
}

impl AddressBookGetter {
    /// `withClient($client)`.
    pub fn with_client(client: DavClient) -> Self {
        AddressBookGetter { client }
    }

    /// Get address book data: uri, capabilities, and name.
    pub async fn execute(mut self) -> Result<AddressBookData, Exception> {
        self.get_address_book_data().await
    }

    /// Get address book data: uri, capabilities, and name.
    async fn get_address_book_data(&mut self) -> Result<AddressBookData, Exception> {
        let uri = self.get_address_book_base_uri().await?;

        self.client.set_base_uri(&uri);

        let capabilities = if uri.starts_with("https://www.googleapis.com") {
            // Google API sucks
            Capabilities {
                addressbook_multiget: true,
                addressbook_query: true,
                sync_collection: true,
                address_data: AddressData { content_type: Some("text/vcard".into()), version: Some("3.0".into()) },
            }
        } else {
            self.get_capabilities().await?
        };

        let name = self.client.get_property("{DAV:}displayname", "").await?;

        Ok(AddressBookData { uri, capabilities, name })
    }

    /// Calculate address book base uri.
    async fn get_address_book_base_uri(&self) -> Result<String, Exception> {
        let mut address_book_url = None;
        let mut path = dav::parse_url(&self.client.path(None)).path;

        while let Some(p) = path.clone().filter(|p| !php_empty(p)) {
            match self.get_address_book_url(&p, 0).await {
                Ok(url) => address_book_url = url,
                // Catch error
                Err(Exception::Request(_)) => {}
                Err(e) => return Err(e),
            }

            if address_book_url.is_some() {
                break;
            }

            path = Some(p.rsplit_once('/').map_or(p.as_str(), |(before, _)| before).to_owned());
        }

        // If no address book found, try to get the principal
        if address_book_url.is_none() {
            match self.get_address_book_for_uri(&self.client.path(None)).await {
                Ok(url) => address_book_url = url,
                // Catch error
                Err(Exception::Request(_)) => {}
                Err(e) => return Err(e),
            }
        }

        // If no address book found, try with the service url
        if address_book_url.is_none() {
            let service_url = self.client.get_service_url().await?;

            if let Some(service_url) = service_url {
                match self.get_address_book_for_uri(&service_url).await {
                    Ok(url) => address_book_url = url,
                    // Catch error
                    Err(Exception::Request(_)) => {}
                    Err(e) => return Err(e),
                }
            }
        }

        let Some(address_book_url) = address_book_url else {
            return Err(Exception::DavClient("No address book found".into()));
        };

        let address_book_url = self.client.path(dav::parse_url(&address_book_url).path.as_deref());

        if !address_book_url.contains("https://www.googleapis.com") {
            // Check the OPTIONS of the server
            self.check_options(true, &address_book_url).await?;
        }

        Ok(address_book_url)
    }

    /// Calculate address book base uri.
    async fn get_address_book_for_uri(&self, uri: &str) -> Result<Option<String>, Exception> {
        // Get the principal of this account
        let principal = self.get_current_user_principal(uri).await?;

        let home = self.get_address_book_home(&principal).await?;

        // Get the AddressBook of this principal
        let address_book = self.get_address_book_url(&home, 1).await?;

        Ok(address_book.map(|address_book| self.client.path(Some(&address_book))))
    }

    /// Check options of the server.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc2518#section-15> and
    /// <https://datatracker.ietf.org/doc/html/rfc6352#section-6.1>.
    async fn check_options(&self, addressbook: bool, url: &str) -> Result<(), Exception> {
        let options = self.client.options(url).await?;

        let has = |o: &str| options.iter().any(|x| x == o);
        if !has("1") || !has("3") || (addressbook && !has("addressbook")) {
            return Err(Exception::DavServerNotCompliant(
                "server is not compliant with rfc2518 section 15.1, or rfc6352 section 6.1".into(),
            ));
        }

        Ok(())
    }

    /// Get principal name.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc5397#section-3>.
    async fn get_current_user_principal(&self, uri: &str) -> Result<String, Exception> {
        let prop = self.client.get_property("{DAV:}current-user-principal", uri).await?;

        match prop {
            Value::Null => Err(Exception::DavServerNotCompliant(
                "Server does not support rfc 5397 section 3 (DAV:current-user-principal)".into(),
            )),
            Value::Text(prop) if php_empty(&prop) => Err(Exception::DavServerNotCompliant(
                "Server does not support rfc 5397 section 3 (DAV:current-user-principal)".into(),
            )),
            Value::Text(prop) => Ok(prop),
            prop => first_value(prop, "getCurrentUserPrincipal"),
        }
    }

    /// Get addressbook url.
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc6352#section-7.1.1>.
    async fn get_address_book_home(&self, principal: &str) -> Result<String, Exception> {
        let prop = self.client.get_property(&format!("{{{NS_CARDDAV}}}addressbook-home-set"), principal).await?;

        match prop {
            Value::Null => Err(Exception::DavServerNotCompliant(
                "Server does not support rfc 6352 section 7.1.1 (CARD:addressbook-home-set)".into(),
            )),
            Value::Text(prop) if php_empty(&prop) => Err(Exception::DavServerNotCompliant(
                "Server does not support rfc 6352 section 7.1.1 (CARD:addressbook-home-set)".into(),
            )),
            Value::Text(prop) => Ok(prop),
            prop => first_value(prop, "getAddressBookHome"),
        }
    }

    /// Get Url for address book.
    async fn get_address_book_url(&self, uri: &str, depth: u8) -> Result<Option<String>, Exception> {
        let path = dav::parse_url(uri).path;

        let books = self.client.prop_find(&["{DAV:}resourcetype".into()], depth, uri).await?;
        let books: Vec<(String, Option<Value>)> = match books {
            PropFind::Properties(properties) => properties.into_iter().map(|(book, p)| (book, Some(p))).collect(),
            PropFind::Hrefs(hrefs) => hrefs
                .into_iter()
                .map(|(book, properties)| (book, dav::get(&properties, "{DAV:}resourcetype").cloned()))
                .collect(),
        };

        for (book, resources) in books {
            if depth != 0 && Some(&book) == path.as_ref() {
                continue;
            }

            let is_address_book = match resources {
                Some(Value::ResourceType(types)) => types.iter().any(|t| *t == format!("{{{NS_CARDDAV}}}addressbook")),
                Some(Value::Text(_)) => {
                    return Err(Exception::Error("Call to a member function is() on string".into()));
                }
                Some(Value::Elements(_)) => {
                    return Err(Exception::Error("Call to a member function is() on array".into()));
                }
                Some(Value::Null) | None => {
                    return Err(Exception::Error("Call to a member function is() on null".into()));
                }
            };
            if is_address_book {
                return Ok(Some(if depth == 0 { uri.to_owned() } else { book }));
            }
        }

        Ok(None)
    }

    /// Get capabilities properties.
    async fn get_capabilities(&self) -> Result<Capabilities, Exception> {
        let (addressbook_multiget, addressbook_query, sync_collection) = self.get_supported_report_set().await?;
        let address_data = self.get_supported_address_data().await?;

        Ok(Capabilities { addressbook_multiget, addressbook_query, sync_collection, address_data })
    }

    /// Get supported-report-set property.
    async fn get_supported_report_set(&self) -> Result<(bool, bool, bool), Exception> {
        let supported_report_set = self.client.get_supported_report_set().await?;

        let has = |report: &str| supported_report_set.iter().any(|r| r.as_deref() == Some(report));
        let addressbook_multiget = has(&format!("{{{NS_CARDDAV}}}addressbook-multiget"));
        let addressbook_query = has(&format!("{{{NS_CARDDAV}}}addressbook-query"));
        let sync_collection = has("{DAV:}sync-collection");

        Ok((addressbook_multiget, addressbook_query, sync_collection))
    }

    /// Get supported-address-data property.
    async fn get_supported_address_data(&self) -> Result<AddressData, Exception> {
        // get the supported card format
        let address_data =
            match self.client.get_property(&format!("{{{NS_CARDDAV}}}supported-address-data"), "").await? {
                Value::Elements(elements) => elements,
                _ => Vec::new(),
            };
        let mut data = address_data.iter().find(|d| d.attribute("version") == Some("4.0"));
        if data.is_none() {
            data = address_data.iter().find(|d| d.attribute("version") == Some("3.0"));
        }

        let Some(data) = data else {
            // It should not happen !
            return Ok(AddressData { content_type: Some("text/vcard".into()), version: Some("4.0".into()) });
        };

        Ok(AddressData {
            content_type: data.attribute("content-type").map(str::to_owned),
            version: data.attribute("version").map(str::to_owned),
        })
    }
}

/// `return $prop[0]['value'];` (a string).
fn first_value(prop: Value, function: &str) -> Result<String, Exception> {
    match prop {
        Value::Elements(elements) => match elements.into_iter().next().map(|e| e.value) {
            Some(Value::Text(value)) => Ok(value),
            _ => Err(Exception::Error(format!("AddressBookGetter::{function}(): Return value must be of type string"))),
        },
        _ => Err(Exception::Error("Cannot use object of type ResourceType as array".into())),
    }
}

/// The local changes of `getLocalChanges()`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalChanges {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// `AddressBookSynchronizer`.
pub struct AddressBookSynchronizer {
    pub(super) subscription: addressbook_subscriptions::Model,
    pub(super) user: users::Model,
    pub(super) client: DavClient,
}

impl AddressBookSynchronizer {
    /// `withSubscription($subscription)`.
    pub async fn with_subscription<C: ConnectionTrait>(
        db: &C,
        config: &Config,
        subscription: addressbook_subscriptions::Model,
    ) -> ApiResult<Self> {
        let user = super::user(db, &subscription).await?;
        Ok(AddressBookSynchronizer { subscription, user, client: DavClient::new(config) })
    }

    /// Sync the address book.
    pub async fn execute(mut self, db: &DatabaseConnection, config: &Config, force: bool) -> Result<String, Exception> {
        self.client = get_client(config, &self.subscription)?;

        let update_sync_token = UpdateSubscriptionLocalSyncToken { addressbook_subscription_id: self.subscription.id };

        let jobs = if force { self.forcesync(db, config).await? } else { self.sync(db, config).await? };

        let then = is_way_push(&self.subscription).then_some(update_sync_token);

        dispatch_batch(db, config, jobs, then).await
    }

    async fn get_local_changes(&self, db: &DatabaseConnection, config: &Config) -> Result<LocalChanges, Exception> {
        let sync_token = self.subscription.sync_token_id.map(|id| id.to_string()).unwrap_or_default();
        let local_changes = backend::get_changes_for_address_book(
            &with_user(db, config, &self.user),
            &self.subscription.vault_id.to_string(),
            Some(&sync_token),
        )
        .await?;

        Ok(local_changes
            .map(|c| LocalChanges { added: c.added, modified: c.modified, deleted: c.deleted })
            .unwrap_or_default())
    }

    /// Sync the address book.
    pub(super) async fn sync(&mut self, db: &DatabaseConnection, config: &Config) -> Result<Vec<Job>, Exception> {
        // Get distant changes to sync
        let changes = self.get_distant_changes(db, config).await?;

        // Get distant contacts
        let mut jobs = Vec::new();
        if is_way_get(&self.subscription) {
            jobs = PrepareJobsContactUpdater::with_subscription(self.subscription.clone()).execute(&changes);
        }

        if is_way_push(&self.subscription) {
            // Get changes to sync
            let local_changes = self.get_local_changes(db, config).await?;

            let pushes = PrepareJobsContactPush::with_subscription(db, self.subscription.clone())
                .await?
                .execute(db, config, &local_changes, Some(&changes))
                .await?;
            logs(&pushes);

            jobs.extend(pushes);
        }

        Ok(jobs)
    }

    /// Sync the address book.
    pub(super) async fn forcesync(&mut self, db: &DatabaseConnection, config: &Config) -> Result<Vec<Job>, Exception> {
        // Get current list of contacts
        let vault_id = self.subscription.vault_id.to_string();
        let local_contacts = backend::card_get_objects(&with_user(db, config, &self.user), Some(&vault_id)).await?;
        let local_uuids: Vec<String> = local_contacts.iter().map(DavResource::id).collect();

        // Get distant changes to sync
        let dist_contacts = self.get_all_contacts_etag().await?;

        // Get missed contacts
        let missed: Vec<ContactDto> =
            dist_contacts.iter().filter(|contact| !local_uuids.contains(&decode_uri(contact.uri()))).cloned().collect();

        let mut jobs = Vec::new();
        if is_way_get(&self.subscription) {
            jobs = PrepareJobsContactUpdater::with_subscription(self.subscription.clone()).execute(&missed);
        }

        if is_way_push(&self.subscription) {
            // Get changes to sync
            let local_changes = self.get_local_changes(db, config).await?;

            let pushes = PrepareJobsContactPushMissed::with_subscription(db, self.subscription.clone())
                .await?
                .execute(db, config, &local_changes, &dist_contacts, &local_contacts)
                .await?;
            logs(&pushes);

            jobs.extend(pushes);
        }

        Ok(jobs)
    }

    /// Filter contacts to only return vcards type and new contacts or contacts with matching etags.
    async fn filter_distant_contacts(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        contact: &StatusList,
        href: &str,
    ) -> Result<bool, Exception> {
        // only return vcards
        if !contact.get("200", "{DAV:}getcontenttype").and_then(Value::text).is_some_and(|t| t.contains("text/vcard")) {
            return Ok(false);
        }

        // only new contact or contact with etag that match
        let ctx = with_user(db, config, &self.user);
        let card = backend::get_card(&ctx, &self.subscription.vault_id.to_string(), href).await?;

        Ok(match card {
            None => true,
            Some(card) => Some(card.etag.as_str()) != contact.get("200", "{DAV:}getetag").and_then(Value::text),
        })
    }

    /// Get distant changes to sync.
    pub(super) async fn get_distant_changes(
        &mut self,
        db: &DatabaseConnection,
        config: &Config,
    ) -> Result<Vec<ContactDto>, Exception> {
        let etags = self.get_distant_etags(db).await?;
        let data = etags.responses;

        let mut updated = Vec::new();
        for (href, contact) in &data {
            if self.filter_distant_contacts(db, config, contact, href).await? {
                let etag = contact.get("200", "{DAV:}getetag").and_then(Value::text).map(str::to_owned);
                updated.push((href.clone(), ContactDto::Contact { uri: href.clone(), etag }));
            }
        }
        let deleted = data
            .iter()
            .filter(|(_, contact)| contact.status == "404")
            .map(|(href, _)| (href.clone(), ContactDto::Delete { uri: href.clone() }));

        Ok(merge(updated, deleted))
    }

    /// Get all contacts etag.
    pub(super) async fn get_all_contacts_etag(&self) -> Result<Vec<ContactDto>, Exception> {
        if !has_capability(&self.subscription, "addressbookQuery") {
            return Ok(Vec::new());
        }

        let query = self.client.addressbook_query(&["{DAV:}getetag".into()]).await?;

        let data = query.responses;

        let updated = data
            .iter()
            .filter(|(_, contact)| contact.status == "200")
            .map(|(href, contact)| {
                let etag = contact.get("200", "{DAV:}getetag").and_then(Value::text).map(str::to_owned);
                (href.clone(), ContactDto::Contact { uri: href.clone(), etag })
            })
            .collect();
        let deleted = data
            .iter()
            .filter(|(_, contact)| contact.status == "404")
            .map(|(href, _)| (href.clone(), ContactDto::Delete { uri: href.clone() }));

        Ok(merge(updated, deleted))
    }

    /// Get refreshed etags.
    ///
    /// The PROPFIND answer is `href => properties`: its entries have no
    /// `properties` nor `status`, so no contact passes the filters of
    /// `getDistantChanges()`.
    async fn get_distant_etags<C: ConnectionTrait>(&mut self, db: &C) -> Result<MultiStatus, Exception> {
        if has_capability(&self.subscription, "syncCollection") {
            // With sync-collection
            self.call_sync_collection_when_needed(db).await
        } else {
            // With PROPFIND
            let hrefs = self.client.prop_find(&["{DAV:}getcontenttype".into(), "{DAV:}getetag".into()], 1, "").await?;
            let PropFind::Hrefs(hrefs) = hrefs else { return Ok(MultiStatus::default()) };
            Ok(MultiStatus {
                responses: hrefs.into_iter().map(|(href, _)| (href, StatusList::default())).collect(),
                synctoken: None,
            })
        }
    }

    /// Make sync-collection request if sync-token has changed.
    async fn call_sync_collection_when_needed<C: ConnectionTrait>(&mut self, db: &C) -> Result<MultiStatus, Exception> {
        // get the current distant syncToken
        let current_sync_token = self.client.get_property("{DAV:}sync-token", "").await?;

        if current_sync_token.text() == Some(self.subscription.distant_sync_token.as_deref().unwrap_or_default()) {
            // no change at all
            return Ok(MultiStatus::default());
        }

        self.call_sync_collection(db).await
    }

    /// Make sync-collection request.
    async fn call_sync_collection<C: ConnectionTrait>(&mut self, db: &C) -> Result<MultiStatus, Exception> {
        let sync_token = self.subscription.distant_sync_token.clone().unwrap_or_default();

        // get sync
        let collection = match self
            .client
            .sync_collection(&["{DAV:}getcontenttype".into(), "{DAV:}getetag".into()], &sync_token)
            .await
        {
            Ok(collection) => {
                // save the new syncToken as current one
                if let Some(new_sync_token) = &collection.synctoken {
                    self.save_distant_sync_token(db, Some(new_sync_token.clone())).await?;
                }
                collection
            }
            Err(e @ Exception::Request(_)) => {
                tracing::error!("AddressBookSynchronizer callSyncCollection:{}", e.message());
                self.save_distant_sync_token(db, None).await?;
                MultiStatus::default()
            }
            Err(e) => return Err(e),
        };

        Ok(collection)
    }

    /// `$this->subscription->distant_sync_token = …; $this->subscription->save();`.
    async fn save_distant_sync_token<C: ConnectionTrait>(&mut self, db: &C, token: Option<String>) -> ApiResult<()> {
        let mut am: addressbook_subscriptions::ActiveModel = self.subscription.clone().into();
        am.distant_sync_token.set_ne(token);
        if am.is_changed() {
            self.subscription = am.update(db).await?;
        }
        Ok(())
    }
}

/// `$updated->merge($deleted)`: by href, a later entry replacing an earlier one.
fn merge(updated: Vec<(String, ContactDto)>, deleted: impl Iterator<Item = (String, ContactDto)>) -> Vec<ContactDto> {
    let mut result = updated;
    for (href, contact) in deleted {
        match result.iter_mut().find(|(h, _)| *h == href) {
            Some(entry) => entry.1 = contact,
            None => result.push((href, contact)),
        }
    }
    result.into_iter().map(|(_, contact)| contact).collect()
}

/// `AddressBookSynchronizer::logs()`.
fn logs(jobs: &[Job]) {
    let updated = jobs.iter().filter(|j| matches!(j, Job::PushVCard(_))).count();
    if updated > 0 {
        tracing::info!("Update or create {updated} card(s) to distant server...");
    }
    let deleted = jobs.iter().filter(|j| matches!(j, Job::DeleteVCard(_))).count();
    if deleted > 0 {
        tracing::info!("Delete {deleted} card(s) to distant server...");
    }
}

/// `PrepareJobsContactPush`.
pub struct PrepareJobsContactPush {
    subscription: addressbook_subscriptions::Model,
    user: users::Model,
}

impl PrepareJobsContactPush {
    /// `withSubscription($subscription)`.
    pub async fn with_subscription<C: ConnectionTrait>(
        db: &C,
        subscription: addressbook_subscriptions::Model,
    ) -> ApiResult<Self> {
        let user = super::user(db, &subscription).await?;
        Ok(PrepareJobsContactPush { subscription, user })
    }

    /// Push contacts to the distant server.
    pub async fn execute(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        local_changes: &LocalChanges,
        changes: Option<&[ContactDto]>,
    ) -> Result<Vec<Job>, Exception> {
        let modified = self
            .prepare_push_changed_contacts(db, config, &local_changes.modified, changes.unwrap_or_default())
            .await?;
        let added = self.prepare_push_added_contacts(db, config, &local_changes.added).await?;
        let deleted = self.prepare_deleted_contacts(db, &local_changes.deleted).await?;

        Ok(modified.into_iter().chain(added).chain(deleted).flatten().collect())
    }

    /// Get list of requests to push new contacts.
    async fn prepare_push_added_contacts(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        contacts: &[String],
    ) -> Result<Vec<Option<Job>>, Exception> {
        // All added contact must be pushed
        let ctx = with_user(db, config, &self.user);
        let mut jobs = Vec::new();
        for uri in self.filter_contacts(db, contacts).await? {
            let card = backend::get_card(&ctx, &self.subscription.vault_id.to_string(), &uri).await?;

            jobs.push(card.map(|card| {
                Job::PushVCard(PushVCard::new(
                    self.subscription.clone(),
                    uri,
                    card.distant_etag,
                    card.carddata,
                    card.contact_id,
                    MODE_MATCH_NONE,
                ))
            }));
        }
        Ok(jobs)
    }

    /// Get list of requests to delete contacts.
    async fn prepare_deleted_contacts<C: ConnectionTrait>(
        &self,
        db: &C,
        contacts: &[String],
    ) -> ApiResult<Vec<Option<Job>>> {
        // All removed contact must be deleted
        Ok(self
            .filter_contacts(db, contacts)
            .await?
            .into_iter()
            .map(|uri| Some(Job::DeleteVCard(DeleteVCard::new(self.subscription.clone(), uri))))
            .collect())
    }

    /// Get list of requests to push modified contacts.
    async fn prepare_push_changed_contacts(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        contacts: &[String],
        changes: &[ContactDto],
    ) -> Result<Vec<Option<Job>>, Exception> {
        let refresh_ids: Vec<String> = changes.iter().map(|contact| decode_uri(contact.uri())).collect();

        // We don't push contact that have just been pulled
        let ctx = with_user(db, config, &self.user);
        let mut jobs = Vec::new();
        for uri in self.filter_contacts(db, contacts).await? {
            if refresh_ids.contains(&decode_uri(&uri)) {
                continue;
            }

            let card = backend::get_card(&ctx, &self.subscription.vault_id.to_string(), &uri).await?;

            let Some(card) = card else {
                jobs.push(None);
                continue;
            };

            let mode = if card.distant_etag.is_some() { MODE_MATCH_ETAG } else { MODE_MATCH_ANY };
            jobs.push(Some(Job::PushVCard(PushVCard::new(
                self.subscription.clone(),
                uri,
                card.distant_etag,
                card.carddata,
                card.contact_id,
                mode,
            ))));
        }
        Ok(jobs)
    }

    /// Filter list of contacts.
    async fn filter_contacts<C: ConnectionTrait>(&self, db: &C, contacts: &[String]) -> ApiResult<Vec<String>> {
        let mut result = Vec::new();
        for uri in contacts {
            if self.get_contact_in_vault(db).await?.as_deref() != Some(decode_uri(uri).as_str()) {
                result.push(uri.clone());
            }
        }
        Ok(result)
    }

    async fn get_contact_in_vault<C: ConnectionTrait>(&self, db: &C) -> ApiResult<Option<String>> {
        let entry = user_vault::Entity::find()
            .filter(user_vault::Column::UserId.eq(self.user.id))
            .filter(user_vault::Column::VaultId.eq(self.subscription.vault_id))
            .one(db)
            .await?;

        Ok(entry.map(|pivot| pivot.contact_id.to_string()))
    }
}

/// `PrepareJobsContactPushMissed`.
pub struct PrepareJobsContactPushMissed {
    subscription: addressbook_subscriptions::Model,
    user: users::Model,
}

impl PrepareJobsContactPushMissed {
    /// `withSubscription($subscription)`.
    pub async fn with_subscription<C: ConnectionTrait>(
        db: &C,
        subscription: addressbook_subscriptions::Model,
    ) -> ApiResult<Self> {
        let user = super::user(db, &subscription).await?;
        Ok(PrepareJobsContactPushMissed { subscription, user })
    }

    /// Push contacts to the distant server.
    pub async fn execute(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        local_changes: &LocalChanges,
        dist_contacts: &[ContactDto],
        local_contacts: &[DavResource],
    ) -> Result<Vec<Job>, Exception> {
        let changes = PrepareJobsContactPush::with_subscription(db, self.subscription.clone())
            .await?
            .execute(db, config, local_changes, None)
            .await?;

        let missings =
            self.prepare_push_missed_contacts(db, config, &local_changes.added, dist_contacts, local_contacts).await?;

        Ok(changes.into_iter().chain(missings).collect())
    }

    /// Get list of requests of missed contacts.
    async fn prepare_push_missed_contacts(
        &self,
        db: &DatabaseConnection,
        config: &Config,
        added: &[String],
        dist_contacts: &[ContactDto],
        local_contacts: &[DavResource],
    ) -> Result<Vec<Job>, Exception> {
        let dist_uuids: Vec<String> = dist_contacts.iter().map(|contact| decode_uri(contact.uri())).collect();
        let added_uuids: Vec<String> = added.iter().map(|uri| decode_uri(uri)).collect();

        let ctx = with_user(db, config, &self.user);
        let mut jobs = Vec::new();
        for resource in local_contacts {
            let id = resource.id();
            if dist_uuids.contains(&id) || added_uuids.contains(&id) {
                continue;
            }

            let card = backend::prepare_card(&ctx, resource).await?;

            let mode = if card.distant_etag.is_some() { MODE_MATCH_ANY } else { MODE_MATCH_NONE };
            jobs.push(Job::PushVCard(PushVCard::new(
                self.subscription.clone(),
                card.uri,
                card.distant_etag,
                card.carddata,
                id,
                mode,
            )));
        }
        Ok(jobs)
    }
}

/// `PrepareJobsContactUpdater`.
pub struct PrepareJobsContactUpdater {
    subscription: addressbook_subscriptions::Model,
}

impl PrepareJobsContactUpdater {
    /// `withSubscription($subscription)`.
    pub fn with_subscription(subscription: addressbook_subscriptions::Model) -> Self {
        PrepareJobsContactUpdater { subscription }
    }

    /// Update local contacts.
    pub fn execute(&self, refresh: &[ContactDto]) -> Vec<Job> {
        if has_capability(&self.subscription, "addressbookMultiget") {
            self.refresh_multiget_contacts(refresh)
        } else {
            self.refresh_simple_get_contacts(refresh)
        }
    }

    /// Get contacts data with addressbook-multiget request.
    fn refresh_multiget_contacts(&self, refresh: &[ContactDto]) -> Vec<Job> {
        let updated: Vec<String> =
            refresh.iter().filter(|c| matches!(c, ContactDto::Contact { .. })).map(|c| c.uri().to_owned()).collect();
        let deleted: Vec<String> =
            refresh.iter().filter(|c| matches!(c, ContactDto::Delete { .. })).map(|c| c.uri().to_owned()).collect();

        let mut jobs = Vec::new();
        if !updated.is_empty() {
            jobs.push(Job::GetMultipleVCard(GetMultipleVCard::new(self.subscription.clone(), updated)));
        }
        if !deleted.is_empty() {
            jobs.push(Job::DeleteMultipleVCard(DeleteMultipleVCard::new(self.subscription.clone(), deleted)));
        }

        jobs
    }

    /// Get contacts data with request.
    fn refresh_simple_get_contacts(&self, refresh: &[ContactDto]) -> Vec<Job> {
        let updated: Vec<&ContactDto> = refresh.iter().filter(|c| matches!(c, ContactDto::Contact { .. })).collect();
        let deleted: Vec<&ContactDto> = refresh.iter().filter(|c| matches!(c, ContactDto::Delete { .. })).collect();

        let mut jobs = Vec::new();
        if !updated.is_empty() {
            tracing::info!("Get {} card(s) from distant server...", updated.len());

            jobs.extend(
                updated
                    .into_iter()
                    .map(|contact| Job::GetVCard(GetVCard::new(self.subscription.clone(), contact.clone()))),
            );
        }
        if !deleted.is_empty() {
            tracing::info!("Delete {} card(s) from distant server...", deleted.len());

            jobs.extend(deleted.into_iter().map(|contact| {
                Job::DeleteLocalVCard(DeleteLocalVCard::new(self.subscription.clone(), contact.uri().to_owned()))
            }));
        }

        jobs
    }
}

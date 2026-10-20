//! `app/Domains/Contact/DavClient/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, EntityTrait};

use super::dav::DavClient;
use super::utils::{AddressBookData, AddressBookGetter, AddressBookSynchronizer};
use super::{Exception, user, with_user};
use crate::config::Config;
use crate::dav::backend::{self, CARD};
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::addressbook_subscriptions;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateAddressBookSubscription`.
pub struct CreateAddressBookSubscription {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub base_uri: String,
    pub username: String,
    pub password: String,
}

impl CreateAddressBookSubscription {
    /// Add a new Adress Book.
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        config: &Config,
    ) -> Result<addressbook_subscriptions::Model, Exception> {
        let mut v = Validator::new();
        if self.base_uri.trim().is_empty() {
            v.fail("base_uri", "required", &[]);
        } else if url::Url::parse(&self.base_uri).map_or(true, |u| u.host_str().is_none()) {
            v.fail("base_uri", "url", &[]);
        }
        if self.username.trim().is_empty() {
            v.fail("username", "required", &[]);
        }
        if self.password.trim().is_empty() {
            v.fail("password", "required", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
        )
        .await?;

        let address_book_data = self.get_address_book_data(config).await?;

        Ok(self.create_address_book(db, config, &checked, address_book_data).await?)
    }

    async fn create_address_book<C: ConnectionTrait>(
        &self,
        db: &C,
        config: &Config,
        checked: &Checked,
        address_book_data: AddressBookData,
    ) -> ApiResult<addressbook_subscriptions::Model> {
        Ok(addressbook_subscriptions::ActiveModel {
            id: Set(Uuid::new()),
            user_id: Set(checked.author.id),
            vault_id: Set(checked.vault().id),
            username: Set(self.username.clone()),
            password: Set(crate::crypto::encrypt(&config.key, &self.password)),
            uri: Set(address_book_data.uri),
            capabilities: Set(serde_json::to_string(&address_book_data.capabilities).map_err(ApiError::internal)?),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }

    async fn get_address_book_data(&self, config: &Config) -> Result<AddressBookData, Exception> {
        let client = self.get_client(config);

        AddressBookGetter::with_client(client).execute().await
    }

    fn get_client(&self, config: &Config) -> DavClient {
        let mut client = DavClient::new(config);
        client.set_base_uri(&self.base_uri).set_credentials(&self.username, &self.password);
        client
    }
}

/// `SynchronizeAddressBook`.
pub struct SynchronizeAddressBook {
    pub account_id: Uuid,
    pub addressbook_subscription_id: Uuid,
    pub force: Option<bool>,
}

impl SynchronizeAddressBook {
    /// The id of the batch that ran, `None` for an inactive subscription.
    pub async fn execute(self, db: &DatabaseConnection, config: &Config) -> Result<Option<String>, Exception> {
        let subscription = self.validate(db).await?;

        let force = self.force.unwrap_or(false);

        synchronize(db, config, subscription, force).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<addressbook_subscriptions::Model> {
        let subscription = addressbook_subscriptions::Entity::find_by_id(self.addressbook_subscription_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        if user(db, &subscription).await?.account_id != self.account_id {
            return Err(ApiError::not_found());
        }

        Ok(subscription)
    }
}

/// `SynchronizeAddressBook::synchronize()`.
async fn synchronize(
    db: &DatabaseConnection,
    config: &Config,
    subscription: addressbook_subscriptions::Model,
    force: bool,
) -> Result<Option<String>, Exception> {
    if !subscription.active {
        return Ok(None);
    }

    let synchronizer = AddressBookSynchronizer::with_subscription(db, config, subscription).await?;
    synchronizer.execute(db, config, force).await.map(Some)
}

/// `UpdateSubscriptionLocalSyncToken`.
pub struct UpdateSubscriptionLocalSyncToken {
    pub addressbook_subscription_id: Uuid,
}

impl UpdateSubscriptionLocalSyncToken {
    pub async fn execute(self, db: &DatabaseConnection, config: &Config) -> Result<(), Exception> {
        let subscription = addressbook_subscriptions::Entity::find_by_id(self.addressbook_subscription_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        update_sync_token(db, config, subscription).await
    }
}

/// Update the synctoken.
async fn update_sync_token(
    db: &DatabaseConnection,
    config: &Config,
    subscription: addressbook_subscriptions::Model,
) -> Result<(), Exception> {
    let ctx = with_user(db, config, &user(db, &subscription).await?);
    let token = backend::get_current_sync_token(&ctx, &CARD, Some(&subscription.vault_id.to_string())).await?;

    if let Some(token) = token {
        let mut am: addressbook_subscriptions::ActiveModel = subscription.into();
        am.sync_token_id.set_ne(Some(token.id));
        if am.is_changed() {
            am.update(db).await?;
        }
    }

    Ok(())
}

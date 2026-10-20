//! `app/Domains/Settings/ManageUserPreferences/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait};

use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::users;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `User::NUMBER_FORMAT_TYPES`.
const NUMBER_FORMAT_TYPES: [&str; 5] = ["locale", "1,234.56", "1 234,56", "1.234,56", "1234.56"];

/// `StoreDateFormatPreference`.
pub struct StoreDateFormatPreference {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub date_format: String,
}

impl StoreDateFormatPreference {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        required(&mut v, "date_format", &self.date_format, 255);
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;

        self.update_user(db, author).await
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.date_format = Set(self.date_format);
        Ok(author.update(db).await?)
    }
}

/// `StoreDistanceFormatPreference`.
pub struct StoreDistanceFormatPreference {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub distance_format: String,
}

impl StoreDistanceFormatPreference {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        required(&mut v, "distance_format", &self.distance_format, 255);
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;

        self.update_user(db, author).await
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.distance_format = Set(self.distance_format);
        Ok(author.update(db).await?)
    }
}

/// `StoreLocale`.
pub struct StoreLocale {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub locale: String,
}

impl StoreLocale {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        if required(&mut v, "locale", &self.locale, 5) && !crate::i18n::is_supported(&self.locale) {
            v.fail("locale", "in", &[]);
        }
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;

        self.update_user(db, author).await
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.locale = Set(self.locale);
        Ok(author.update(db).await?)
    }
}

/// `StoreMapsPreference`.
pub struct StoreMapsPreference {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub maps_site: String,
}

impl StoreMapsPreference {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        required(&mut v, "maps_site", &self.maps_site, 255);
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;

        self.update_user(db, author).await
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.default_map_site = Set(self.maps_site);
        Ok(author.update(db).await?)
    }
}

/// `StoreNameOrderPreference`.
pub struct StoreNameOrderPreference {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name_order: String,
}

impl StoreNameOrderPreference {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        required(&mut v, "name_order", &self.name_order, 255);
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;
        self.check_name_order_validity()?;

        self.update_user(db, author).await
    }

    fn check_name_order_validity(&self) -> ApiResult<()> {
        if self.name_order.matches('%').count() < 1 {
            return Err(ApiError::field("name_order", "invalid", "The name order must contain at least one variable."));
        }

        if self.name_order.matches('%').count() % 2 == 1 {
            return Err(ApiError::field(
                "name_order",
                "invalid",
                "At least one % is missing to have a valid name order.",
            ));
        }
        Ok(())
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.name_order = Set(self.name_order);
        Ok(author.update(db).await?)
    }
}

/// `StoreNumberFormatPreference`.
pub struct StoreNumberFormatPreference {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub number_format: String,
}

impl StoreNumberFormatPreference {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        if self.number_format.trim().is_empty() {
            v.fail("number_format", "required", &[]);
        } else {
            v.one_of("number_format", Some(&self.number_format), &NUMBER_FORMAT_TYPES);
        }
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;

        self.update_user(db, author).await
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.number_format = Set(self.number_format);
        Ok(author.update(db).await?)
    }
}

/// `StoreTimezone`.
pub struct StoreTimezone {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub timezone: String,
}

impl StoreTimezone {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        required(&mut v, "timezone", &self.timezone, 255);
        v.finish()?;
        let author = author(db, self.account_id, self.author_id).await?;

        self.update_user(db, author).await
    }

    async fn update_user<C: ConnectionTrait>(self, db: &C, author: users::Model) -> ApiResult<users::Model> {
        let mut author: users::ActiveModel = author.into();
        author.timezone = Set(Some(self.timezone));
        Ok(author.update(db).await?)
    }
}

/// `required|string|max:n`; whether the value is present.
fn required(v: &mut Validator, field: &str, value: &str, max: usize) -> bool {
    if value.trim().is_empty() {
        v.fail(field, "required", &[]);
        return false;
    }
    v.max_len(field, Some(value), max);
    true
}

/// `author_must_belong_to_account`: the author (`$this->author`).
async fn author<C: ConnectionTrait>(db: &C, account_id: Uuid, author_id: Uuid) -> ApiResult<users::Model> {
    Ok(check(db, &Check { account_id, author_id, ..Default::default() }).await?.author)
}

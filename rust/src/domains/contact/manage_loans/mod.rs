//! `app/Domains/Contact/ManageLoans/Services`.

#[cfg(test)]
mod tests;

use chrono::{NaiveDate, NaiveTime};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionSession,
    TransactionTrait,
};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check, contact_in_vault, value_or_null};
use crate::entities::{contact_loan, contacts, currencies, loans};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateLoan`.
pub struct CreateLoan {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub currency_id: Option<Id>,
    pub r#type: String,
    pub name: String,
    pub description: Option<String>,
    pub loaner_ids: Vec<Uuid>,
    pub loanee_ids: Vec<Uuid>,
    pub amount_lent: Option<i32>,
    pub loaned_at: Option<String>,
}

impl CreateLoan {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<loans::Model> {
        let (checked, loaners, loanees) = self.validate(db).await?;
        let loan = self.create(db, &loaners, &loanees).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(loan)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(Checked, Vec<contacts::Model>, Vec<contacts::Model>)> {
        let mut v = Validator::new();
        if let Some(id) = self.currency_id
            && currencies::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("currency_id", "exists", &[]);
        }
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.max_len("type", Some(&self.r#type), 255);
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 65535);
        v.max_len("description", self.description.as_deref(), 65535);
        if self.loaner_ids.is_empty() {
            v.fail("loaner_ids", "required", &[]);
        }
        if self.loanee_ids.is_empty() {
            v.fail("loanee_ids", "required", &[]);
        }
        date_format(&mut v, "loaned_at", self.loaned_at.as_deref());
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let mut loaners = Vec::new();
        for id in &self.loaner_ids {
            loaners.push(contact_in_vault(db, checked.vault().id, *id).await?);
        }

        let mut loanees = Vec::new();
        for id in &self.loanee_ids {
            loanees.push(contact_in_vault(db, checked.vault().id, *id).await?);
        }

        Ok((checked, loaners, loanees))
    }

    async fn create<C: ConnectionTrait>(
        self,
        db: &C,
        loaners: &[contacts::Model],
        loanees: &[contacts::Model],
    ) -> ApiResult<loans::Model> {
        let loan = loans::ActiveModel {
            vault_id: Set(self.vault_id),
            r#type: Set(self.r#type),
            name: Set(self.name),
            description: Set(value_or_null(self.description)),
            amount_lent: Set(self.amount_lent.filter(|a| *a != 0)),
            currency_id: Set(self.currency_id),
            loaned_at: Set(value_or_null(self.loaned_at).map(|d| as_date_time(&d)).transpose()?),
            ..Default::default()
        }
        .insert(db)
        .await?;

        for loaner in loaners {
            for loanee in loanees {
                sync_without_detaching(db, loan.id, loaner.id, loanee.id, contact_loan::Column::LoanerId).await?;
            }
        }

        for loanee in loanees {
            for loaner in loaners {
                sync_without_detaching(db, loan.id, loaner.id, loanee.id, contact_loan::Column::LoaneeId).await?;
            }
        }

        Ok(loan)
    }
}

/// `DestroyLoan`.
pub struct DestroyLoan {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub loan_id: Id,
}

impl DestroyLoan {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let loan = find(db, checked.vault().id, self.loan_id).await?;

        loans::Entity::delete_by_id(loan.id).exec(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(())
    }
}

/// `ToggleLoan`.
pub struct ToggleLoan {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub loan_id: Id,
}

impl ToggleLoan {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<loans::Model> {
        let (checked, loan) = self.validate(db).await?;
        Self::toggle(db, &checked, loan).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, loans::Model)> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let loan = find(db, checked.vault().id, self.loan_id).await?;
        Ok((checked, loan))
    }

    async fn toggle<C: ConnectionTrait>(db: &C, checked: &Checked, loan: loans::Model) -> ApiResult<loans::Model> {
        let settled = !loan.settled;
        let mut loan: loans::ActiveModel = loan.into();
        loan.settled = Set(settled);
        // Q6: un-settling clears `settled_at` (the PHP sets it to now either way).
        loan.settled_at = Set(settled.then(crate::db::now));
        let loan = loan.update(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(loan)
    }
}

/// `UpdateLoan`.
pub struct UpdateLoan {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub loan_id: Id,
    pub currency_id: Option<Id>,
    pub r#type: String,
    pub name: String,
    pub description: Option<String>,
    pub loaner_ids: Vec<Uuid>,
    pub loanee_ids: Vec<Uuid>,
    pub amount_lent: Option<i32>,
    pub loaned_at: Option<String>,
}

impl UpdateLoan {
    pub async fn execute<C: ConnectionTrait + TransactionTrait>(self, db: &C) -> ApiResult<loans::Model> {
        let (checked, loan, loaners, loanees) = self.validate(db).await?;
        self.update(db, &checked, loan, &loaners, &loanees).await
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(Checked, loans::Model, Vec<contacts::Model>, Vec<contacts::Model>)> {
        let mut v = Validator::new();
        if let Some(id) = self.currency_id
            && currencies::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("currency_id", "exists", &[]);
        }
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.max_len("type", Some(&self.r#type), 255);
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 65535);
        v.max_len("description", self.description.as_deref(), 65535);
        if self.loaner_ids.is_empty() {
            v.fail("loaner_ids", "required", &[]);
        }
        if self.loanee_ids.is_empty() {
            v.fail("loanee_ids", "required", &[]);
        }
        date_format(&mut v, "loaned_at", self.loaned_at.as_deref());
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let loan = find(db, checked.vault().id, self.loan_id).await?;

        let mut loaners = Vec::new();
        for id in &self.loaner_ids {
            loaners.push(contact_in_vault(db, checked.vault().id, *id).await?);
        }

        let mut loanees = Vec::new();
        for id in &self.loanee_ids {
            loanees.push(contact_in_vault(db, checked.vault().id, *id).await?);
        }

        Ok((checked, loan, loaners, loanees))
    }

    async fn update<C: ConnectionTrait + TransactionTrait>(
        self,
        db: &C,
        checked: &Checked,
        loan: loans::Model,
        loaners: &[contacts::Model],
        loanees: &[contacts::Model],
    ) -> ApiResult<loans::Model> {
        let mut loan: loans::ActiveModel = loan.into();
        loan.r#type = Set(self.r#type);
        loan.name = Set(self.name);
        loan.description = Set(value_or_null(self.description));
        loan.amount_lent = Set(self.amount_lent.filter(|a| *a != 0));
        loan.loaned_at = Set(value_or_null(self.loaned_at).map(|d| as_date_time(&d)).transpose()?);
        loan.currency_id = Set(self.currency_id);
        let loan = loan.update(db).await?;

        let txn = db.begin().await?;
        // remove all the current loaners and loanees
        contact_loan::Entity::delete_many().filter(contact_loan::Column::LoanId.eq(loan.id)).exec(&txn).await?;

        for loaner in loaners {
            for loanee in loanees {
                sync_without_detaching(&txn, loan.id, loaner.id, loanee.id, contact_loan::Column::LoanerId).await?;
            }
        }

        for loanee in loanees {
            for loaner in loaners {
                sync_without_detaching(&txn, loan.id, loaner.id, loanee.id, contact_loan::Column::LoaneeId).await?;
            }
        }
        txn.commit().await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(loan)
    }
}

/// `$this->vault->loans()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, vault_id: Uuid, loan_id: Id) -> ApiResult<loans::Model> {
    loans::Entity::find_by_id(loan_id)
        .filter(loans::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$loaner->loansAsLoaner()` (`key` is `loaner_id`) or `$loanee->loansAsLoanee()`
/// (`key` is `loanee_id`) `->syncWithoutDetaching([$loan->id => [<other key> => …]])`:
/// inserts the row when the contact has none for the loan, else sets the
/// other key on its rows.
async fn sync_without_detaching<C: ConnectionTrait>(
    db: &C,
    loan_id: Id,
    loaner_id: Uuid,
    loanee_id: Uuid,
    key: contact_loan::Column,
) -> ApiResult<()> {
    let (contact_id, other, other_id) = match key {
        contact_loan::Column::LoanerId => (loaner_id, contact_loan::Column::LoaneeId, loanee_id),
        _ => (loanee_id, contact_loan::Column::LoanerId, loaner_id),
    };
    let attached = contact_loan::Entity::find()
        .filter(key.eq(contact_id))
        .filter(contact_loan::Column::LoanId.eq(loan_id))
        .one(db)
        .await?;
    if attached.is_none() {
        contact_loan::Entity::insert(contact_loan::ActiveModel {
            loan_id: Set(loan_id),
            loaner_id: Set(loaner_id),
            loanee_id: Set(loanee_id),
            ..Default::default()
        })
        .exec_without_returning(db)
        .await?;
    } else {
        contact_loan::Entity::update_many()
            .col_expr(other, Expr::value(other_id))
            .filter(key.eq(contact_id))
            .filter(contact_loan::Column::LoanId.eq(loan_id))
            .exec(db)
            .await?;
    }
    Ok(())
}

/// `nullable|date_format:Y-m-d`: a non-blank value parses and formats back
/// to itself.
fn date_format(v: &mut Validator, field: &str, value: Option<&str>) {
    if let Some(value) = value
        && !value.trim().is_empty()
        && !NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok_and(|d| d.format("%Y-%m-%d").to_string() == value)
    {
        v.fail(field, "date_format", &[("format", "Y-m-d")]);
    }
}

/// The `datetime` cast of a `Y-m-d` string: its midnight.
fn as_date_time(value: &str) -> ApiResult<Timestamp> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|d| Timestamp(d.and_time(NaiveTime::MIN)))
        .map_err(ApiError::internal)
}

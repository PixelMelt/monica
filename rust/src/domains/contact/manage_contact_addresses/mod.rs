//! `app/Domains/Contact/ManageContactAddresses/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect, RelationTrait, sea_query::JoinType,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::{addresses, contact_address, contact_feed_items, contacts};
use crate::error::{ApiError, ApiResult};

/// `AssociateAddressToContact`.
pub struct AssociateAddressToContact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub address_id: Id,
    pub is_past_address: bool,
}

impl AssociateAddressToContact {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<addresses::Model> {
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

        let address = find_address(db, &checked, self.address_id).await?;

        contact_address::ActiveModel {
            contact_id: Set(checked.contact().id),
            address_id: Set(address.id),
            is_past_address: Set(self.is_past_address),
            ..Default::default()
        }
        .insert(db)
        .await?;

        create_feed_item(db, &checked, &address, "address_created").await?;

        Ok(address)
    }
}

/// `RemoveAddressFromContact`.
pub struct RemoveAddressFromContact {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub address_id: Id,
}

impl RemoveAddressFromContact {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<addresses::Model> {
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

        let address = find_address(db, &checked, self.address_id).await?;

        contact_address::Entity::delete_many()
            .filter(contact_address::Column::ContactId.eq(checked.contact().id))
            .filter(contact_address::Column::AddressId.eq(address.id))
            .exec(db)
            .await?;

        let contacts = contact_address::Entity::find()
            .join(JoinType::InnerJoin, contact_address::Relation::Contacts.def())
            .filter(contact_address::Column::AddressId.eq(address.id))
            .filter(contacts::Column::DeletedAt.is_null())
            .count(db)
            .await?;
        if contacts == 0 {
            addresses::Entity::delete_by_id(address.id).exec(db).await?;
        }

        create_feed_item(db, &checked, &address, "address_destroyed").await?;

        Ok(address)
    }
}

/// `$this->vault->addresses()->findOrFail($id)`.
async fn find_address<C: ConnectionTrait>(db: &C, checked: &Checked, id: Id) -> ApiResult<addresses::Model> {
    addresses::Entity::find_by_id(id)
        .filter(addresses::Column::VaultId.eq(checked.vault().id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    checked: &Checked,
    address: &addresses::Model,
    action: &str,
) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = checked.contact().clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;

    contact_feed_items::ActiveModel {
        author_id: Set(Some(checked.author.id)),
        contact_id: Set(checked.contact().id),
        action: Set(action.into()),
        description: Set(Some(get_address_as_string(address))),
        feedable_type: Set(Some("App\\Models\\Address".into())),
        feedable_id: Set(Some(address.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `MapHelper::getAddressAsString`.
fn get_address_as_string(address: &addresses::Model) -> String {
    let mut sentence = String::new();

    if let Some(line_1) = &address.line_1 {
        sentence = line_1.clone();
    }
    for part in [&address.line_2, &address.city, &address.province, &address.postal_code, &address.country]
        .into_iter()
        .flatten()
    {
        sentence.push(' ');
        sentence.push_str(part);
    }

    // trim extra whitespaces inside the address
    let mut out = String::with_capacity(sentence.len());
    for c in sentence.chars() {
        let space = matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r');
        if !space {
            out.push(c);
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out
}

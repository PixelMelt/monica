//! `tests/Unit/Domains/Settings/CreateAccount/Services`.

use sea_orm::{ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter};

use super::*;
use crate::entities::{
    address_types, call_reason_types, genders, modules, pet_categories, pronouns, relationship_group_types,
    user_notification_channels,
};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `assertDatabaseHas`.
async fn assert_has<E: EntityTrait>(db: &TestDb, condition: Condition) {
    let row = E::find().filter(condition.clone()).one(&db.conn).await.unwrap();
    assert!(row.is_some(), "no {} row with {condition:?}", E::default().table_name());
}

// ---- CreateAccountTest

#[tokio::test]
async fn it_creates_an_account() {
    let db = TestDb::migrated().await;

    let user = CreateAccount {
        first_name: "john".into(),
        last_name: "john".into(),
        username: " John ".into(),
        password: Some("john".into()),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let account = accounts::Entity::find_by_id(user.account_id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(account.storage_limit_in_mb, 0, "no storage limit");

    let row = users::Entity::find_by_id(user.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, user.account_id);
    assert_eq!(row.first_name.as_deref(), Some("john"));
    assert_eq!(row.last_name.as_deref(), Some("john"));
    assert_eq!(row.username, "john");
    assert!(row.is_account_administrator);
    assert_eq!(row.timezone.as_deref(), Some("UTC"));

    // `Queue::assertPushed(SetupAccount::class, author_id === $user->id)`: it ran for the user.
    assert_has::<templates::Entity>(&db, Condition::all().add(templates::Column::AccountId.eq(user.account_id))).await;
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let err =
        CreateAccount { first_name: String::new(), last_name: String::new(), username: String::new(), password: None }
            .execute(&db.conn)
            .await
            .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

// ---- SetupAccountTest

#[tokio::test]
async fn it_sets_an_account_up() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;

    SetupAccount { account_id: user.account_id, author_id: user.id }.execute(&db.conn).await.unwrap();

    let currency = currencies::Entity::find().one(&db.conn).await.unwrap().unwrap();

    assert_has::<account_currency::Entity>(
        &db,
        Condition::all()
            .add(account_currency::Column::CurrencyId.eq(currency.id))
            .add(account_currency::Column::AccountId.eq(user.account_id)),
    )
    .await;
    assert_eq!(currencies::Entity::find().count(&db.conn).await.unwrap(), 164);

    // No email channel: the user adds a Pushover channel themselves.
    let channels = user_notification_channels::Entity::find()
        .filter(user_notification_channels::Column::UserId.eq(user.id))
        .count(&db.conn)
        .await
        .unwrap();
    assert_eq!(channels, 0);

    assert_has::<templates::Entity>(
        &db,
        Condition::all()
            .add(templates::Column::AccountId.eq(user.account_id))
            .add(templates::Column::NameTranslationKey.eq("Default template"))
            .add(templates::Column::CanBeDeleted.eq(false)),
    )
    .await;
    assert_has::<template_pages::Entity>(
        &db,
        Condition::all()
            .add(template_pages::Column::NameTranslationKey.eq("Contact information"))
            .add(template_pages::Column::CanBeDeleted.eq(false)),
    )
    .await;
    assert_has::<template_pages::Entity>(
        &db,
        Condition::all()
            .add(template_pages::Column::NameTranslationKey.eq("Life & goals"))
            .add(template_pages::Column::CanBeDeleted.eq(true)),
    )
    .await;
    for name_translation_key in [
        "Notes",
        "Contact name",
        "Avatar",
        "Contact feed",
        "Gender and pronoun",
        "Labels",
        "Reminders",
        "Loans",
        "Job information",
        "Religions",
        "Tasks",
        "Calls",
        "Pets",
        "Goals",
        "Life",
        "Family summary",
        "Addresses",
        "Groups",
        "Posts",
    ] {
        assert_has::<modules::Entity>(
            &db,
            Condition::all()
                .add(modules::Column::AccountId.eq(user.account_id))
                .add(modules::Column::NameTranslationKey.eq(name_translation_key)),
        )
        .await;
    }

    assert_has::<relationship_group_types::Entity>(
        &db,
        Condition::all()
            .add(relationship_group_types::Column::NameTranslationKey.eq("Love"))
            .add(relationship_group_types::Column::CanBeDeleted.eq(false))
            .add(relationship_group_types::Column::Type.eq("love")),
    )
    .await;
    assert_has::<relationship_group_types::Entity>(
        &db,
        Condition::all()
            .add(relationship_group_types::Column::NameTranslationKey.eq("Family"))
            .add(relationship_group_types::Column::CanBeDeleted.eq(false))
            .add(relationship_group_types::Column::Type.eq("family")),
    )
    .await;
    assert_has::<relationship_group_types::Entity>(
        &db,
        Condition::all()
            .add(relationship_group_types::Column::NameTranslationKey.eq("Work"))
            .add(relationship_group_types::Column::CanBeDeleted.eq(true)),
    )
    .await;
    assert_has::<relationship_group_types::Entity>(
        &db,
        Condition::all()
            .add(relationship_group_types::Column::NameTranslationKey.eq("Friend"))
            .add(relationship_group_types::Column::CanBeDeleted.eq(true)),
    )
    .await;

    for name_translation_key in ["Male", "Other", "Female"] {
        assert_has::<genders::Entity>(
            &db,
            Condition::all()
                .add(genders::Column::AccountId.eq(user.account_id))
                .add(genders::Column::NameTranslationKey.eq(name_translation_key)),
        )
        .await;
    }

    for name_translation_key in ["he/him", "she/her", "they/them", "per/per", "ve/ver", "xe/xem", "ze/hir"] {
        assert_has::<pronouns::Entity>(
            &db,
            Condition::all()
                .add(pronouns::Column::AccountId.eq(user.account_id))
                .add(pronouns::Column::NameTranslationKey.eq(name_translation_key)),
        )
        .await;
    }

    assert_has::<contact_information_types::Entity>(
        &db,
        Condition::all()
            .add(contact_information_types::Column::AccountId.eq(user.account_id))
            .add(contact_information_types::Column::NameTranslationKey.eq("Email address"))
            .add(contact_information_types::Column::Protocol.eq("mailto:")),
    )
    .await;
    assert_has::<contact_information_types::Entity>(
        &db,
        Condition::all()
            .add(contact_information_types::Column::AccountId.eq(user.account_id))
            .add(contact_information_types::Column::NameTranslationKey.eq("Phone"))
            .add(contact_information_types::Column::Protocol.eq("tel:")),
    )
    .await;

    assert_has::<address_types::Entity>(
        &db,
        Condition::all()
            .add(address_types::Column::AccountId.eq(user.account_id))
            .add(address_types::Column::NameTranslationKey.eq("🏡 Home")),
    )
    .await;

    assert_has::<pet_categories::Entity>(
        &db,
        Condition::all()
            .add(pet_categories::Column::AccountId.eq(user.account_id))
            .add(pet_categories::Column::NameTranslationKey.eq("Dog")),
    )
    .await;

    for label_translation_key in ["Personal", "Business"] {
        assert_has::<call_reason_types::Entity>(
            &db,
            Condition::all()
                .add(call_reason_types::Column::AccountId.eq(user.account_id))
                .add(call_reason_types::Column::LabelTranslationKey.eq(label_translation_key)),
        )
        .await;
    }

    for (label_translation_key, position) in
        [("Birthday", 1), ("Anniversary", 2), ("Christmas", 3), ("Just because", 4), ("Wedding", 5)]
    {
        assert_has::<gift_occasions::Entity>(
            &db,
            Condition::all()
                .add(gift_occasions::Column::LabelTranslationKey.eq(label_translation_key))
                .add(gift_occasions::Column::Position.eq(position)),
        )
        .await;
    }

    assert_has::<religions::Entity>(
        &db,
        Condition::all().add(religions::Column::TranslationKey.eq("Christian")).add(religions::Column::Position.eq(1)),
    )
    .await;
    db.drop().await;
}

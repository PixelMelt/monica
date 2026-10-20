//! `app/Console/Commands/Local/SetupDummyAccount.php` (`monica:dummy`, K006):
//! prepares an account with fake data so users can play with it.

use anyhow::Context;
use chrono::{DateTime, Datelike, Days, Months, Utc};
use fake::Fake;
use fake::faker::chrono::en::DateTimeBetween;
use fake::faker::lorem::en::{Paragraph, Sentence, Word};
use fake::faker::name::en::{FirstName, LastName};
use rand::random_range;
use rand::seq::{IndexedRandom, SliceRandom};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm_migration::MigratorTrait;

use crate::config::Config;
use crate::domains::contact::manage_contact::CreateContact;
use crate::domains::contact::manage_contact_important_dates::CreateContactImportantDate;
use crate::domains::contact::manage_goals::{CreateGoal, ToggleStreak};
use crate::domains::contact::manage_groups::{AddContactToGroup, CreateGroup};
use crate::domains::contact::manage_notes::CreateNote;
use crate::domains::contact::manage_tasks::CreateContactTask;
use crate::domains::settings::create_account::CreateAccount;
use crate::domains::vault::manage_journals::{CreateJournal, CreatePost};
use crate::domains::vault::manage_vault::CreateVault;
use crate::entities::{
    contact_important_date_types, contacts, group_type_roles, group_types, post_templates, users, vaults,
};
use crate::error::Code;

/// `Vault::TYPE_PERSONAL`.
const VAULT_TYPE_PERSONAL: &str = "personal";

/// `ContactImportantDate::TYPE_BIRTHDATE`.
const TYPE_BIRTHDATE: &str = "birthdate";

/// `handle`. `fresh` is the opposite of `--migrate`: drop every table and
/// migrate (`migrate:fresh`) rather than apply pending migrations. `force` is
/// `--force`.
///
/// The PHP asks "Are you sure you want to proceed?" in every environment
/// unless `--force` is given (`confirmToProceed($warning, true)`). A function
/// can't ask: the CLI asks before calling it when `--force` is absent and
/// passes `force: true` on a yes. Without `force`, it runs outside production;
/// in production it prints the warning and "Command cancelled." and returns
/// `Ok` without touching the database, as declining the prompt does.
pub async fn setup_dummy_account(
    db: &DatabaseConnection,
    config: &Config,
    fresh: bool,
    force: bool,
) -> anyhow::Result<()> {
    if !start(config, force) {
        return Ok(());
    }
    wipe_and_migrate_db(db, fresh).await?;
    let first_user = create_first_user(db).await?;
    create_vaults(db, &first_user).await?;
    create_contacts(db, &first_user).await?;
    create_groups(db, &first_user).await?;
    create_notes(db, &first_user).await?;
    create_tasks(db, &first_user).await?;
    create_goals(db, &first_user).await?;
    create_journals(db, &first_user).await?;
    create_second_user(db).await?;
    stop(config);
    Ok(())
}

fn start(config: &Config, force: bool) -> bool {
    if config.is_production() && !force {
        println!("Are you sure you want to proceed? This will delete ALL data in your environment.");
        println!("Command cancelled.");
        return false;
    }

    println!("This process will take a few minutes to complete. Be patient and read a book in the meantime.");
    true
}

async fn wipe_and_migrate_db(db: &DatabaseConnection, fresh: bool) -> anyhow::Result<()> {
    println!("☐ Migration of the database");
    if !fresh {
        crate::db::migrate(db).await?;
    } else {
        crate::db::migrations::Migrator::fresh(db).await?;
    }
    Ok(())
}

fn stop(config: &Config) {
    println!();
    println!("-----------------------------");
    println!("|");
    println!("| Welcome to Monica");
    println!("|");
    println!("-----------------------------");
    println!("| You can now sign in with one of these two accounts:");
    println!("| An account with a lot of data:");
    println!("| username: admin");
    println!("| password: admin123");
    println!("|----------------------------");
    println!("|A blank account:");
    println!("| username: blank");
    println!("| password: blank123");
    println!("|----------------------------");
    println!("| URL:      {}", config.url);
    println!("-----------------------------");

    println!("Setup is done. Have fun.");
}

async fn create_first_user(db: &DatabaseConnection) -> anyhow::Result<users::Model> {
    println!("☐ Create first user of the account");

    Ok(CreateAccount {
        username: "admin".into(),
        password: Some("admin123".into()),
        first_name: "Michael".into(),
        last_name: "Scott".into(),
    }
    .execute(db)
    .await?)
}

async fn create_vaults(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create vaults");

    let mut i = 0;
    while i < random_range(3..=5) {
        CreateVault {
            account_id: first_user.account_id,
            author_id: first_user.id,
            template_id: None,
            r#type: VAULT_TYPE_PERSONAL.into(),
            name: FirstName().fake(),
            description: (random_range(1..=2) == 1).then(|| sentence(6)),
        }
        .execute(db)
        .await?;
        i += 1;
    }
    Ok(())
}

async fn create_contacts(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create contacts");

    for vault in vaults::Entity::find().all(db).await? {
        let birth_date_type = contact_important_date_types::Entity::find()
            .filter(contact_important_date_types::Column::VaultId.eq(vault.id))
            .filter(contact_important_date_types::Column::InternalType.eq(TYPE_BIRTHDATE))
            .one(db)
            .await?
            .context("the vault has no birthdate type")?;

        let mut i = 0;
        while i < random_range(2..=13) {
            let now = Utc::now();
            let birth_date: DateTime<Utc> = DateTimeBetween(now - Months::new(100 * 12), now).fake();

            let contact = CreateContact {
                account_id: first_user.account_id,
                id: None,
                vault_id: vault.id,
                author_id: first_user.id,
                first_name: Some(FirstName().fake()),
                last_name: Some(LastName().fake()),
                middle_name: (random_range(1..=2) == 1).then(|| LastName().fake()),
                nickname: None,
                maiden_name: None,
                prefix: None,
                suffix: None,
                gender_id: None,
                pronoun_id: None,
                template_id: None,
                listed: true,
            }
            .execute(db)
            .await?;

            CreateContactImportantDate {
                account_id: first_user.account_id,
                vault_id: vault.id,
                author_id: first_user.id,
                contact_id: contact.id,
                contact_important_date_type_id: Some(birth_date_type.id),
                label: "Birthdate".into(),
                day: Some(birth_date.day() as i32),
                month: Some(birth_date.month() as i32),
                year: (random_range(1..=2) == 1).then(|| birth_date.year()),
            }
            .execute(db)
            .await?;
            i += 1;
        }
    }
    Ok(())
}

async fn create_groups(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create groups");

    for vault in vaults::Entity::find().all(db).await? {
        let mut i = 0;
        while i < random_range(2..=5) {
            let group_type = if random_range(1..=2) == 1 {
                let group_types = group_types::Entity::find()
                    .filter(group_types::Column::AccountId.eq(first_user.account_id))
                    .all(db)
                    .await?;
                Some(group_types.choose(&mut rand::rng()).cloned().context("the account has no group types")?)
            } else {
                None
            };
            let group = CreateGroup {
                account_id: first_user.account_id,
                vault_id: vault.id,
                author_id: first_user.id,
                group_type_id: group_type.as_ref().map(|t| t.id),
                name: Some(Word().fake()),
            }
            .execute(db)
            .await?;

            // Add random contacts to the group
            let mut contacts = contacts::Entity::live().filter(contacts::Column::VaultId.eq(vault.id)).all(db).await?;
            contacts.shuffle(&mut rand::rng());
            contacts.truncate(random_range(1..=5));

            for contact in contacts {
                let group_type_roles = match &group_type {
                    Some(group_type) if random_range(1..=2) == 1 => Some(
                        group_type_roles::Entity::find()
                            .filter(group_type_roles::Column::GroupTypeId.eq(group_type.id))
                            .all(db)
                            .await?,
                    ),
                    _ => None,
                };
                let role = group_type_roles.and_then(|roles| roles.choose(&mut rand::rng()).cloned());

                AddContactToGroup {
                    account_id: first_user.account_id,
                    vault_id: vault.id,
                    author_id: first_user.id,
                    group_id: group.id,
                    contact_id: contact.id,
                    group_type_role_id: role.map(|r| r.id),
                }
                .execute(db)
                .await?;
            }
            i += 1;
        }
    }
    Ok(())
}

async fn create_notes(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create notes");

    for contact in contacts::Entity::live().all(db).await? {
        for _ in 0..4 {
            CreateNote {
                account_id: first_user.account_id,
                vault_id: contact.vault_id,
                author_id: first_user.id,
                contact_id: contact.id,
                emotion_id: None,
                title: (random_range(1..=2) == 1).then(|| sentence(random_range(3..=6))),
                body: Paragraph(2..6).fake(),
            }
            .execute(db)
            .await?;
        }
    }
    Ok(())
}

async fn create_tasks(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create tasks");

    for contact in contacts::Entity::live().all(db).await? {
        for _ in 0..4 {
            CreateContactTask {
                account_id: first_user.account_id,
                vault_id: contact.vault_id,
                author_id: first_user.id,
                contact_id: contact.id,
                label: sentence(random_range(3..=6)),
                description: None,
                due_at: None,
            }
            .execute(db)
            .await?;
        }
    }
    Ok(())
}

async fn create_goals(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create goals");

    let goals = ["Lose 5 kgs", "Practice sport every day", "Develop Monica every day", "Kiss my wife"];

    for contact in contacts::Entity::live().all(db).await? {
        for goal in goals.iter().take(random_range(1..=4)) {
            let goal = CreateGoal {
                account_id: first_user.account_id,
                vault_id: contact.vault_id,
                author_id: first_user.id,
                contact_id: contact.id,
                name: Some((*goal).into()),
            }
            .execute(db)
            .await?;

            for _ in 0..4 {
                let mut date = Utc::now().date_naive() - Months::new(2 * 12);
                let mut j = 0;
                while j < random_range(1..=20) {
                    date = date + Days::new(random_range(1..=3));

                    let toggled = ToggleStreak {
                        account_id: first_user.account_id,
                        vault_id: contact.vault_id,
                        author_id: first_user.id,
                        contact_id: contact.id,
                        goal_id: Some(goal.id),
                        happened_at: date.format("%Y-%m-%d").to_string(),
                    }
                    .execute(db)
                    .await;
                    if let Err(e) = toggled
                        && e.code != Code::Conflict
                    {
                        return Err(e.into());
                    }
                    j += 1;
                }
            }
        }
    }
    Ok(())
}

async fn create_journals(db: &DatabaseConnection, first_user: &users::Model) -> anyhow::Result<()> {
    println!("☐ Create journals");

    let journals = ["Road trip", "My private diary", "Journal of 2022", "Incredible stories"];

    for vault in vaults::Entity::find().all(db).await? {
        for journal in journals.iter().take(random_range(1..=4)) {
            let journal = CreateJournal {
                account_id: first_user.account_id,
                vault_id: vault.id,
                author_id: first_user.id,
                name: (*journal).into(),
                description: (random_range(1..=2) == 1).then(|| sentence(6)),
            }
            .execute(db)
            .await?;

            let mut j = 0;
            while j < random_range(1..=20) {
                let post_templates = post_templates::Entity::find()
                    .filter(post_templates::Column::AccountId.eq(first_user.account_id))
                    .all(db)
                    .await?;
                let post_template_id =
                    post_templates.choose(&mut rand::rng()).context("the account has no post templates")?.id;
                let now = Utc::now();
                let written_at: DateTime<Utc> = DateTimeBetween(now - Days::new(now.ordinal0().into()), now).fake();

                CreatePost {
                    account_id: first_user.account_id,
                    vault_id: vault.id,
                    author_id: first_user.id,
                    journal_id: journal.id,
                    post_template_id,
                    title: Some(sentence(6)),
                    published: false,
                    written_at: Some(written_at.format("%Y-%m-%d").to_string()),
                }
                .execute(db)
                .await?;
                j += 1;
            }
        }
    }
    Ok(())
}

async fn create_second_user(db: &DatabaseConnection) -> anyhow::Result<()> {
    println!("☐ Create second user of the account");

    CreateAccount {
        username: "blank".into(),
        password: Some("blank123".into()),
        first_name: "John".into(),
        last_name: "Doe".into(),
    }
    .execute(db)
    .await?;
    Ok(())
}

/// Faker's `sentence($nbWords)`: `$nbWords` give or take 40%.
fn sentence(nb_words: usize) -> String {
    Sentence(nb_words * 60 / 100 + 1..nb_words * 140 / 100 + 2).fake()
}

#[cfg(test)]
mod tests {
    use sea_orm::PaginatorTrait;

    use super::*;
    use crate::entities::{contact_tasks, goals, journals, notes, posts, templates, user_vault};
    use crate::testing::TestDb;

    #[tokio::test]
    async fn it_sets_up_the_dummy_and_the_blank_accounts() {
        let db = TestDb::migrated().await;
        let config = Config::for_tests("", std::env::temp_dir());

        setup_dummy_account(&db.conn, &config, false, false).await.unwrap();

        let users = users::Entity::find().all(&db.conn).await.unwrap();
        let first = users.iter().find(|u| u.username == "admin").expect("first user");
        let second = users.iter().find(|u| u.username == "blank").expect("second user");
        assert_eq!(users.len(), 2);
        assert_ne!(first.account_id, second.account_id);
        assert_eq!((first.first_name.as_deref(), first.last_name.as_deref()), (Some("Michael"), Some("Scott")));
        assert_eq!((second.first_name.as_deref(), second.last_name.as_deref()), (Some("John"), Some("Doe")));
        for user in [first, second] {
            assert!(user.is_account_administrator);
        }
        assert!(crate::password::verify("admin123", first.password.as_deref().unwrap()));
        assert!(crate::password::verify("blank123", second.password.as_deref().unwrap()));

        let vaults = vaults::Entity::find().all(&db.conn).await.unwrap();
        assert!((3..=5).contains(&vaults.len()), "{} vaults", vaults.len());
        for vault in &vaults {
            assert_eq!(vault.account_id, first.account_id);
            assert_eq!(vault.r#type, "personal");
            let contacts =
                contacts::Entity::live().filter(contacts::Column::VaultId.eq(vault.id)).all(&db.conn).await.unwrap();
            // The author's own contact, then 2 to 13 more.
            assert!((3..=14).contains(&contacts.len()), "{} contacts", contacts.len());
            for contact in &contacts {
                let notes = notes::Entity::find()
                    .filter(notes::Column::ContactId.eq(contact.id))
                    .count(&db.conn)
                    .await
                    .unwrap();
                assert_eq!(notes, 4);
                let tasks = contact_tasks::Entity::live()
                    .filter(contact_tasks::Column::ContactId.eq(contact.id))
                    .count(&db.conn)
                    .await
                    .unwrap();
                assert_eq!(tasks, 4);
                let goals = goals::Entity::find()
                    .filter(goals::Column::ContactId.eq(contact.id))
                    .count(&db.conn)
                    .await
                    .unwrap();
                assert!((1..=4).contains(&goals), "{goals} goals");
            }
            let journals =
                journals::Entity::find().filter(journals::Column::VaultId.eq(vault.id)).all(&db.conn).await.unwrap();
            assert!((1..=4).contains(&journals.len()), "{} journals", journals.len());
            for journal in &journals {
                let posts = posts::Entity::find()
                    .filter(posts::Column::JournalId.eq(journal.id))
                    .count(&db.conn)
                    .await
                    .unwrap();
                assert!((1..=20).contains(&posts), "{posts} posts");
            }
        }

        // The blank account: set up (a default template), no vault.
        let templates = templates::Entity::find()
            .filter(templates::Column::AccountId.eq(second.account_id))
            .count(&db.conn)
            .await
            .unwrap();
        assert_eq!(templates, 1);
        assert!(!vaults.iter().any(|v| v.account_id == second.account_id));
        let vault_users =
            user_vault::Entity::find().filter(user_vault::Column::UserId.eq(second.id)).count(&db.conn).await.unwrap();
        assert_eq!(vault_users, 0);

        db.drop().await;
    }
}

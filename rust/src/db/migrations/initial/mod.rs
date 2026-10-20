//! The schema built by `database/migrations`, one module per Laravel
//! migration that creates tables, in the same order. Each module defines its
//! tables in their final state: columns, indexes and foreign keys that later
//! Laravel migrations added are folded in, in chronological order, under a
//! comment naming that migration.

use sea_orm::{ConnectionTrait, DbErr};

use crate::db::blueprint::{Engine, Schema};

mod m2013_04_25_132851_create_accounts_table;
mod m2013_04_25_155842_create_attributes_table;
mod m2014_10_12_000000_create_users_table;
mod m2014_10_12_000010_create_vaults_table;
mod m2014_10_12_100000_create_password_resets_table;
mod m2018_12_29_135516_create_synctokens;
mod m2019_05_05_194746_create_crons;
mod m2019_08_19_000000_create_failed_jobs_table;
mod m2019_12_14_000001_create_personal_access_tokens_table;
mod m2020_02_17_224235_create_genders_table;
mod m2020_02_19_173445_create_pronouns_table;
mod m2020_03_20_213318_create_address_types_table;
mod m2020_04_23_133132_create_companies_table;
mod m2020_04_25_133132_create_contacts_table;
mod m2020_04_26_215133_create_addresses_table;
mod m2021_10_09_204235_create_group_table;
mod m2021_10_16_184625_create_relationship_types_table;
mod m2021_10_18_000002_create_pets_table;
mod m2021_10_19_022411_create_contact_feed_table;
mod m2021_10_19_192432_create_labels_table;
mod m2021_10_20_004100_create_contact_fields_table;
mod m2021_10_20_163535_create_emotions_table;
mod m2021_10_21_013005_create_notes_table;
mod m2022_01_22_183321_create_jobs_table;
mod m2022_02_09_145139_create_contact_date_table;
mod m2022_02_18_215852_create_reminders_table;
mod m2022_02_24_002342_create_files_table;
mod m2022_03_22_180407_create_currencies_table;
mod m2022_03_23_005751_create_loans_table;
mod m2022_05_13_201216_create_contact_tasks_table;
mod m2022_05_16_184121_create_call_reasons_table;
mod m2022_05_16_193917_create_calls_table;
mod m2022_05_17_155546_create_life_events_table;
mod m2022_06_02_011219_create_goals_table;
mod m2022_06_09_173049_create_gifts_table;
mod m2022_07_31_200647_create_sessions_table;
mod m2022_07_31_200800_create_cache_table;
mod m2022_09_20_183401_create_journal_table;
mod m2022_09_22_111510_create_posts_table;
mod m2022_10_30_202904_create_religions_table;
mod m2022_11_16_221702_create_job_batches_table;
mod m2022_12_15_004442_create_slices_of_life_table;
mod m2023_01_07_005110_create_mood_tracking_parameters_table;
mod m2023_01_08_155554_create_mood_tracking_table;
mod m2023_02_07_022607_create_vault_quick_facts_template_table;
mod m2023_03_16_182310_create_post_metrics_table;
mod m2023_03_31_125903_create_life_metrics_table;
mod m2023_07_03_230200_create_addressbook_subscription;
mod m2023_08_30_202650_create_logs_table;

/// Every table, in creation order.
pub fn schema(e: Engine) -> Schema {
    let mut s = Schema::default();
    m2013_04_25_132851_create_accounts_table::up(&mut s, e);
    m2013_04_25_155842_create_attributes_table::up(&mut s, e);
    m2014_10_12_000000_create_users_table::up(&mut s, e);
    m2014_10_12_000010_create_vaults_table::up(&mut s, e);
    m2014_10_12_100000_create_password_resets_table::up(&mut s, e);
    m2018_12_29_135516_create_synctokens::up(&mut s, e);
    m2019_05_05_194746_create_crons::up(&mut s, e);
    m2019_08_19_000000_create_failed_jobs_table::up(&mut s, e);
    m2019_12_14_000001_create_personal_access_tokens_table::up(&mut s, e);
    m2020_02_17_224235_create_genders_table::up(&mut s, e);
    m2020_02_19_173445_create_pronouns_table::up(&mut s, e);
    m2020_03_20_213318_create_address_types_table::up(&mut s, e);
    m2020_04_23_133132_create_companies_table::up(&mut s, e);
    m2020_04_25_133132_create_contacts_table::up(&mut s, e);
    m2020_04_26_215133_create_addresses_table::up(&mut s, e);
    m2021_10_09_204235_create_group_table::up(&mut s, e);
    m2021_10_16_184625_create_relationship_types_table::up(&mut s, e);
    m2021_10_18_000002_create_pets_table::up(&mut s, e);
    m2021_10_19_022411_create_contact_feed_table::up(&mut s, e);
    m2021_10_19_192432_create_labels_table::up(&mut s, e);
    m2021_10_20_004100_create_contact_fields_table::up(&mut s, e);
    m2021_10_20_163535_create_emotions_table::up(&mut s, e);
    m2021_10_21_013005_create_notes_table::up(&mut s, e);
    m2022_01_22_183321_create_jobs_table::up(&mut s, e);
    m2022_02_09_145139_create_contact_date_table::up(&mut s, e);
    m2022_02_18_215852_create_reminders_table::up(&mut s, e);
    m2022_02_24_002342_create_files_table::up(&mut s, e);
    m2022_03_22_180407_create_currencies_table::up(&mut s, e);
    m2022_03_23_005751_create_loans_table::up(&mut s, e);
    m2022_05_13_201216_create_contact_tasks_table::up(&mut s, e);
    m2022_05_16_184121_create_call_reasons_table::up(&mut s, e);
    m2022_05_16_193917_create_calls_table::up(&mut s, e);
    m2022_05_17_155546_create_life_events_table::up(&mut s, e);
    m2022_06_02_011219_create_goals_table::up(&mut s, e);
    m2022_06_09_173049_create_gifts_table::up(&mut s, e);
    m2022_07_31_200647_create_sessions_table::up(&mut s, e);
    m2022_07_31_200800_create_cache_table::up(&mut s, e);
    m2022_09_20_183401_create_journal_table::up(&mut s, e);
    m2022_09_22_111510_create_posts_table::up(&mut s, e);
    m2022_10_30_202904_create_religions_table::up(&mut s, e);
    m2022_11_16_221702_create_job_batches_table::up(&mut s, e);
    m2022_12_15_004442_create_slices_of_life_table::up(&mut s, e);
    m2023_01_07_005110_create_mood_tracking_parameters_table::up(&mut s, e);
    m2023_01_08_155554_create_mood_tracking_table::up(&mut s, e);
    m2023_02_07_022607_create_vault_quick_facts_template_table::up(&mut s, e);
    m2023_03_16_182310_create_post_metrics_table::up(&mut s, e);
    m2023_03_31_125903_create_life_metrics_table::up(&mut s, e);
    m2023_07_03_230200_create_addressbook_subscription::up(&mut s, e);
    m2023_08_30_202650_create_logs_table::up(&mut s, e);
    s
}

/// Rows the Laravel migrations insert.
pub async fn seed<C: ConnectionTrait>(db: &C, e: Engine) -> Result<(), DbErr> {
    m2022_03_22_180407_create_currencies_table::seed(db, e).await
}

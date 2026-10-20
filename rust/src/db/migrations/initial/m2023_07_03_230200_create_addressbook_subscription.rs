//! `database/migrations/2023_07_03_230200_create_addressbook_subscription.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("addressbook_subscriptions", |t| {
        t.uuid("id");
        t.primary(&["id"]);
        t.foreign_id_for("user_id", Key::Uuid).constrained("users").cascade_on_delete();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();

        t.string_len("uri", 2096);
        t.string_len("username", 1024);
        t.string_len("password", 2048);
        t.boolean("active").default(true);
        // AddressBookSubscription::WAY_BOTH
        t.unsigned_tiny_integer("sync_way").default(3);
        t.string_len("capabilities", 2048);
        t.string_len("distant_sync_token", 512).nullable();
        t.string("last_batch").nullable();
        t.unsigned_big_integer("current_logid").nullable();
        t.foreign_id_for("sync_token_id", Key::Int).nullable().constrained("sync_tokens").null_on_delete();
        t.small_integer("frequency").default(180); // 3 hours
        t.timestamp("last_synchronized_at").nullable();
        t.timestamps();

        t.foreign(&["last_batch"]).references(&["id"]).on("job_batches").null_on_delete();
        // 2023_08_30_202650_fix_addressbook_currentlogid: adds current_logid
        // only when missing; it is already there.
        // 2025_07_06_200000_fix_synctokens: drops and re-adds the same
        // sync_token_id foreign key (not on SQLite); MySQL keeps and reuses
        // its index, so nothing changes.
    });
}

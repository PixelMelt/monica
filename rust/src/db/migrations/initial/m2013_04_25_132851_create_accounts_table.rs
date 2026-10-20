//! `database/migrations/2013_04_25_132851_create_accounts_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("accounts", |t| {
        t.uuid("id");
        t.primary(&["id"]);
        t.integer("storage_limit_in_mb").default(0);
        t.timestamps();
    });
}

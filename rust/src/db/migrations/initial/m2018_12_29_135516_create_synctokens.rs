//! `database/migrations/2018_12_29_135516_create_synctokens.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("sync_tokens", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.foreign_id_for("user_id", Key::Uuid).constrained("users").cascade_on_delete();
        t.string("name").default("contacts");
        t.timestamp("timestamp");
        t.timestamps();

        t.index(&["account_id", "user_id", "name"]);
    });
}

//! `database/migrations/2014_10_12_100000_create_password_resets_table.php`
//! (`auth.passwords.users.table` is `password_resets`).

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("password_resets", |t| {
        t.string("email").index();
        t.string("token");
        t.timestamp("created_at").nullable();
    });
}

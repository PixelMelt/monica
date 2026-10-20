//! `database/migrations/2022_07_31_200800_create_cache_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("cache", |t| {
        // `string('key')->primary()`
        t.string("key");
        t.primary(&["key"]);
        t.medium_text("value");
        t.integer("expiration");
    });

    s.create("cache_locks", |t| {
        // `string('key')->primary()`
        t.string("key");
        t.primary(&["key"]);
        t.string("owner");
        t.integer("expiration");
    });
}

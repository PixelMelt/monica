//! `database/migrations/2021_10_19_022411_create_contact_feed_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("contact_feed_items", |t| {
        t.id();
        t.foreign_id_for("author_id", Key::Uuid).nullable().constrained("users").null_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.string("action");
        t.string("description").nullable();
        t.nullable_numeric_morphs("feedable");
        t.timestamps();
    });
}

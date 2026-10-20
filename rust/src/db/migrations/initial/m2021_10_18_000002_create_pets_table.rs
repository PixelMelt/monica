//! `database/migrations/2021_10_18_000002_create_pets_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("pet_categories", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.timestamps();
    });

    s.create("pets", |t| {
        t.id();
        t.uuid("uuid").nullable();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("pet_category_id", Key::Int)
            .constrained("pet_categories")
            .cascade_on_delete();
        t.string("name").nullable();
        t.timestamps();
    });
}

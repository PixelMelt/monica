//! `database/migrations/2021_10_20_004100_create_contact_fields_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("contact_information_types", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.string("protocol").nullable();
        t.boolean("can_be_deleted").default(true);
        t.string("type").nullable();
        t.timestamps();
    });

    s.create("contact_information", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("type_id", Key::Int)
            .constrained("contact_information_types")
            .cascade_on_delete();
        t.string("data");
        t.timestamps();
        // 2025_07_07_213621_add_contact_information_kind
        t.string("kind").nullable();
        t.boolean("pref").default(true);
    });
}

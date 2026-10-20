//! `database/migrations/2021_10_16_184625_create_relationship_types_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("relationship_group_types", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.string("type").nullable();
        t.boolean("can_be_deleted").default(true);
        t.timestamps();
    });

    s.create("relationship_types", |t| {
        t.id();
        t.foreign_id_for("relationship_group_type_id", Key::Int)
            .constrained("relationship_group_types")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.string("name_reverse_relationship").nullable();
        t.string("name_reverse_relationship_translation_key").nullable();
        t.string("type").nullable();
        t.boolean("can_be_deleted").default(true);
        t.timestamps();
    });

    s.create("relationships", |t| {
        t.id();
        t.foreign_id_for("relationship_type_id", Key::Int).constrained("relationship_types").cascade_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("related_contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.timestamps();
    });
}

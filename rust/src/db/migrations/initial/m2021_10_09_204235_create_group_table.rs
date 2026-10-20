//! `database/migrations/2021_10_09_204235_create_group_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, e: Engine) {
    s.create("group_types", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position");
        t.timestamps();
    });

    s.create("group_type_roles", |t| {
        t.id();
        t.foreign_id_for("group_type_id", Key::Int).constrained("group_types").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position").nullable();
        t.timestamps();
    });

    s.create("groups", |t| {
        t.id();
        t.uuid("uuid").nullable();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.foreign_id_for("group_type_id", Key::Int).nullable().constrained("group_types").null_on_delete();
        t.string("name");

        t.medium_text("vcard").nullable();
        t.string_len("distant_uuid", 256).nullable();
        t.string_len("distant_etag", 256).nullable();
        t.string_len("distant_uri", 2096).nullable();

        t.soft_deletes();
        t.timestamps();

        if e.full_text() {
            t.full_text(&["name"]);
        }
    });

    s.create("contact_group", |t| {
        t.foreign_id_for("group_id", Key::Int).constrained("groups").cascade_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("group_type_role_id", Key::Int).nullable().constrained("group_type_roles").null_on_delete();
        t.timestamps();
    });
}

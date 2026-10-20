//! `database/migrations/2022_02_09_145139_create_contact_date_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("contact_important_date_types", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.string("label");
        t.string("internal_type").nullable();
        t.boolean("can_be_deleted").default(true);
        t.timestamps();
    });

    s.create("contact_important_dates", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("contact_important_date_type_id", Key::Int)
            .nullable()
            .constrained("contact_important_date_types")
            .null_on_delete();
        t.string("label");
        t.integer("day").nullable();
        t.integer("month").nullable();
        t.integer("year").nullable();
        t.timestamps();
        // 2025_08_09_223220_update_contact_important_date_and_tasks
        t.uuid("uuid").nullable();
        t.medium_text("vcalendar").nullable();
        t.string_len("distant_uuid", 256).nullable();
        t.string_len("distant_etag", 256).nullable();
        t.string_len("distant_uri", 2096).nullable();
        t.soft_deletes();
        t.index(&["uuid"]);
    });
}

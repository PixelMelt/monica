//! `database/migrations/2022_05_13_201216_create_contact_tasks_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("contact_tasks", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("author_id", Key::Uuid)
            .nullable()
            .constrained("users")
            .null_on_delete();
        t.string("author_name");
        t.string("label");
        t.text("description").nullable();
        t.boolean("completed").default(false);
        t.datetime("completed_at").nullable();
        t.datetime("due_at").nullable();
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

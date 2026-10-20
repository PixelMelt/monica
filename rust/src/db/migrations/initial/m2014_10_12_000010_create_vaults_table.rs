//! `database/migrations/2014_10_12_000010_create_vaults_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("vaults", |t| {
        t.uuid("id");
        t.primary(&["id"]);
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("type");
        t.string("name");
        t.string("description").nullable();
        t.foreign_id_for("default_template_id", Key::Int)
            .nullable()
            .constrained("templates")
            .null_on_delete();
        t.string("default_activity_tab").default("activity");
        t.boolean("show_group_tab").default(true);
        t.boolean("show_tasks_tab").default(true);
        t.boolean("show_files_tab").default(true);
        t.boolean("show_journal_tab").default(true);
        t.boolean("show_companies_tab").default(true);
        t.boolean("show_reports_tab").default(true);
        t.boolean("show_calendar_tab").default(true);
        t.timestamps();
    });
}

//! `database/migrations/2022_02_18_215852_create_reminders_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("contact_reminders", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.string("label");
        t.integer("day").nullable();
        t.integer("month").nullable();
        t.integer("year").nullable();
        t.string("type");
        t.integer("frequency_number").nullable();
        t.datetime("last_triggered_at").nullable();
        t.integer("number_times_triggered").default(0);
        t.timestamps();
    });

    s.create("user_notification_channels", |t| {
        t.id();
        t.foreign_id_for("user_id", Key::Uuid).nullable();
        t.string("type");
        t.string("label").nullable();
        t.text("content");
        t.time("preferred_time").nullable();
        t.boolean("active").default(false);
        t.datetime("verified_at").nullable();
        t.string("verification_token").nullable();
        t.timestamps();
        // 2024_05_12_170430_add_fails_to_notification_channels
        t.integer("fails").default(0);
    });

    s.create("contact_reminder_scheduled", |t| {
        t.id();
        t.foreign_id_for("user_notification_channel_id", Key::Int)
            .constrained("user_notification_channels")
            .cascade_on_delete();
        t.foreign_id_for("contact_reminder_id", Key::Int)
            .constrained("contact_reminders")
            .cascade_on_delete();
        t.datetime("scheduled_at");
        t.datetime("triggered_at").nullable();
        t.timestamps();
    });

    s.create("user_notification_sent", |t| {
        t.id();
        t.foreign_id_for("user_notification_channel_id", Key::Int)
            .constrained("user_notification_channels")
            .cascade_on_delete();
        t.datetime("sent_at");
        t.string("subject_line");
        t.text("payload").nullable();
        t.timestamps();
        // 2024_05_12_170430_add_fails_to_notification_channels
        t.long_text("error").nullable();
    });
}

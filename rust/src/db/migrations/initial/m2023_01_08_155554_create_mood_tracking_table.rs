//! `database/migrations/2023_01_08_155554_create_mood_tracking_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("mood_tracking_events", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("mood_tracking_parameter_id", Key::Int)
            .constrained("mood_tracking_parameters")
            .cascade_on_delete();
        t.datetime("rated_at");
        t.text("note").nullable();
        t.integer("number_of_hours_slept").nullable();
        t.timestamps();
    });
}

//! `database/migrations/2022_06_02_011219_create_goals_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("goals", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.string("name");
        t.boolean("active").default(false);
        t.timestamps();
    });

    s.create("streaks", |t| {
        t.id();
        t.foreign_id_for("goal_id", Key::Int)
            .constrained("goals")
            .cascade_on_delete();
        t.datetime("happened_at");
        t.timestamps();
    });
}

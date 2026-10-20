//! `database/migrations/2023_03_31_125903_create_life_metrics_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("life_metrics", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.string("label");
        t.timestamps();
    });

    s.create("contact_life_metric", |t| {
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("life_metric_id", Key::Int)
            .constrained("life_metrics")
            .cascade_on_delete();
        t.timestamps();
    });
}

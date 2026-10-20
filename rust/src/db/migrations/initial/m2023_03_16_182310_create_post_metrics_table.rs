//! `database/migrations/2023_03_16_182310_create_post_metrics_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("journal_metrics", |t| {
        t.id();
        t.unsigned_big_integer("journal_id");
        t.string("label");
        t.timestamps();
        t.foreign(&["journal_id"]).references(&["id"]).on("journals").on_delete("cascade");
    });

    s.create("post_metrics", |t| {
        t.id();
        t.unsigned_big_integer("post_id");
        t.unsigned_big_integer("journal_metric_id");
        t.integer("value");
        t.string("label").nullable();
        t.timestamps();
        t.foreign(&["post_id"]).references(&["id"]).on("posts").on_delete("cascade");
        t.foreign(&["journal_metric_id"]).references(&["id"]).on("journal_metrics").on_delete("cascade");
    });
}

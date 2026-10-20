//! `database/migrations/2022_11_16_221702_create_job_batches_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("job_batches", |t| {
        // `string('id')->primary()`
        t.string("id");
        t.primary(&["id"]);
        t.string("name");
        t.integer("total_jobs");
        t.integer("pending_jobs");
        t.integer("failed_jobs");
        t.text("failed_job_ids");
        t.medium_text("options").nullable();
        t.integer("cancelled_at").nullable();
        t.integer("created_at");
        t.integer("finished_at").nullable();
    });
}

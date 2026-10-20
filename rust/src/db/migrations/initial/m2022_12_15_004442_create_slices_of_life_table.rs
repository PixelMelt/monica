//! `database/migrations/2022_12_15_004442_create_slices_of_life_table.php`
//! (its `posts.slice_of_life_id` is folded into `posts`).

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("slice_of_lives", |t| {
        t.id();
        t.foreign_id_for("journal_id", Key::Int)
            .constrained("journals")
            .cascade_on_delete();
        t.foreign_id_for("file_cover_image_id", Key::Int)
            .nullable()
            .constrained("files")
            .null_on_delete();
        t.string("name");
        t.string("description").nullable();
        t.timestamps();
    });
}

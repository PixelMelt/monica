use sea_orm_migration::prelude::*;

use crate::db::blueprint::Engine;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250101_000001_initial"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let e = Engine::from_backend(manager.get_database_backend());
        for sql in super::initial::schema(e).statements(e) {
            db.execute_unprepared(&sql).await?;
        }
        super::initial::seed(db, e).await
    }
}

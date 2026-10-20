//! `tests/Unit/Domains/Contact/ManageTasks/Dav`.

use super::super::tests::*;
use super::*;
use crate::domains::base::VaultPermission;
use crate::testing::{TestDb, factory};

// ---- ImportContactTaskTest

#[tokio::test]
async fn it_imports_uuid_default() {
    let db = TestDb::migrated().await;
    let author = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &author, VaultPermission::Edit).await;
    let context = vcalendar_context(&author, &vault);

    let mut vcalendar = Document::vcalendar(vec![]).unwrap();
    let vtodo = vcalendar
        .create_component("VTODO", vec![("UID", "31fdc242-c974-436e-98de-6b21624d6e34".into())], true)
        .unwrap();
    vcalendar.add(Node::Component(vtodo));

    let id = ImportContactTask.import_uid(&context, None, &vcalendar);

    assert_eq!(id.as_deref(), Some("31fdc242-c974-436e-98de-6b21624d6e34"));
    db.drop().await;
}

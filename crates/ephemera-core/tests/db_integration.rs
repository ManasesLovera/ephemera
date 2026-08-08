// Integration test against a real Postgres instance. Requires DATABASE_URL to point
// at a reachable database (docker-compose.yml at the repo root, or the `postgres`
// service container in CI). If unreachable, the test is skipped rather than failed —
// this suite must not block `cargo test` on a machine without Docker running.

use ephemera_core::db_store::DbStore;
use ephemera_core::types::{FileMeta, Origin};

fn test_db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://ephemera:ephemera_dev_only@localhost:5432/ephemera".to_string()
    })
}

#[tokio::test]
async fn insert_list_and_delete_round_trip() {
    let db = DbStore::connect(&test_db_url()).await;
    if !db.is_connected() {
        eprintln!(
            "skipping: no reachable Postgres ({:?})",
            db.offline_reason()
        );
        return;
    }

    let meta = FileMeta {
        id: format!("test-{}", uuid::Uuid::new_v4()),
        name: "integration-test.bin".to_string(),
        size: 5,
        mime: "application/octet-stream".to_string(),
        created_at: 0,
        origin: Origin::Upload,
    };
    let bytes = b"hello";

    db.insert(&meta, bytes, Origin::Ram)
        .await
        .expect("insert should succeed");

    let files = db.list().await.expect("list should succeed");
    assert!(files.iter().any(|f| f.meta.id == meta.id));

    let logical = db
        .logical_bytes()
        .await
        .expect("logical_bytes should succeed");
    assert!(logical >= 5);

    db.remove(&meta.id).await.expect("delete should succeed");
    let files_after = db.list().await.expect("list should succeed");
    assert!(!files_after.iter().any(|f| f.meta.id == meta.id));
}

#[tokio::test]
async fn quota_is_enforced_against_logical_bytes() {
    let db = DbStore::connect(&test_db_url()).await;
    if !db.is_connected() {
        eprintln!(
            "skipping: no reachable Postgres ({:?})",
            db.offline_reason()
        );
        return;
    }

    let meta = FileMeta {
        id: format!("test-{}", uuid::Uuid::new_v4()),
        name: "too-big.bin".to_string(),
        size: 200 * 1024 * 1024, // over the 100 MB cap
        mime: "application/octet-stream".to_string(),
        created_at: 0,
        origin: Origin::Upload,
    };
    let result = db.insert(&meta, b"x", Origin::Ram).await;
    assert!(
        result.is_err(),
        "expected quota rejection for an oversized file"
    );
}

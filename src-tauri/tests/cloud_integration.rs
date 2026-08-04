// Integration test against the real GCS bucket. Requires a service-account key at
// GCS_KEY_PATH (default: gcs-key.json in this crate's directory) — see
// docs/09-gcs-tier.md. If the key is absent (e.g. in CI, which has no credentials),
// the test is skipped rather than failed.

use ephemera_lib::cloud_store::CloudStore;
use ephemera_lib::types::{FileMeta, Origin};

#[tokio::test]
async fn upload_list_and_delete_round_trip() {
    let key_path = std::env::var("GCS_KEY_PATH").unwrap_or_else(|_| "gcs-key.json".to_string());
    let bucket =
        std::env::var("GCS_BUCKET").unwrap_or_else(|_| "ephemera-vault-alterna".to_string());
    let cloud = CloudStore::load(&key_path, bucket);

    if !cloud.is_connected() {
        eprintln!(
            "skipping: no GCS credentials ({:?})",
            cloud.offline_reason()
        );
        return;
    }

    let meta = FileMeta {
        id: format!("test-{}", uuid::Uuid::new_v4()),
        name: "integration-test.txt".to_string(),
        size: 13,
        mime: "text/plain".to_string(),
        created_at: 0,
        origin: Origin::Upload,
    };

    let uploaded = cloud
        .upload(&meta, b"hello, world!".to_vec())
        .await
        .expect("upload should succeed against a real bucket");

    let files = cloud.list().await.expect("list should succeed");
    assert!(files.iter().any(|f| f.object_name == uploaded.object_name));

    let bytes_used = cloud.bytes_used().await.expect("bytes_used should succeed");
    assert!(bytes_used >= 13);

    cloud
        .remove(&uploaded.object_name)
        .await
        .expect("delete should succeed");
    let files_after = cloud.list().await.expect("list should succeed");
    assert!(!files_after
        .iter()
        .any(|f| f.object_name == uploaded.object_name));
}

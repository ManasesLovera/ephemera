// End-to-end vault-path persistence: launch 1 calls `set_vault_path` (the same
// public entry point the UI would), launch 2 reads the OS config dir back via
// `config_file::load` — the exact pair of operations main.rs performs at
// startup. Offline db/cloud stores are fine: `set_vault_path` never touches
// them. The database URL points at port 1 on purpose so the test stays
// hermetic even on machines where the dev Postgres is running.

use ephemera_core::config_file;
use ephemera_core::state::AppState;

#[tokio::test]
async fn set_vault_path_persists_across_simulated_relaunch() {
    let tmp = tempfile::tempdir().unwrap();
    let config_home = tmp.path().join("config-home");
    std::env::set_var("XDG_CONFIG_HOME", &config_home);

    let db = ephemera_core::db_store::DbStore::connect("postgres://127.0.0.1:1/unreachable").await;
    let missing_key = tmp.path().join("missing-gcs-key.json");
    let cloud = ephemera_core::cloud_store::CloudStore::load(
        missing_key.to_str().unwrap(),
        "unused-bucket".to_string(),
    );
    let state = AppState::new(tmp.path().join("first-vault"), db, cloud);

    // Launch 1: the user picks a new vault folder.
    let chosen = tmp.path().join("chosen-vault");
    ephemera_core::set_vault_path(&state, chosen.to_str().unwrap()).unwrap();

    // The config file landed in the OS config dir — and provably NOT inside
    // the vault, where it would corrupt the disk-usage accounting.
    let config_path = config_file::config_file_path().unwrap();
    assert!(config_path.starts_with(&config_home));
    assert!(!config_path.starts_with(&chosen));
    let written = std::fs::read_to_string(&config_path).unwrap();
    assert!(written.contains("chosen-vault"));

    // Launch 2: startup reads the same config dir and gets the choice back.
    let persisted = config_file::load().expect("second launch must find the config");
    assert_eq!(persisted.vault_path, chosen.to_string_lossy());

    std::env::remove_var("XDG_CONFIG_HOME");
}

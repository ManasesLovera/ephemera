pub mod cloud_store;
pub mod commands;
pub mod db_store;
pub mod error;
pub mod metrics;
pub mod ram_store;
pub mod state;
pub mod stream;
pub mod types;
pub mod vault;

use state::AppState;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let vault_path = app
                .path()
                .app_data_dir()
                .expect("app data dir must resolve")
                .join("vault");

            tauri::async_runtime::block_on(async {
                let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://ephemera:ephemera_dev_only@localhost:5432/ephemera".to_string()
                });
                let db = db_store::DbStore::connect(&database_url).await;

                let gcs_key_path =
                    std::env::var("GCS_KEY_PATH").unwrap_or_else(|_| "gcs-key.json".to_string());
                let gcs_bucket =
                    std::env::var("GCS_BUCKET").unwrap_or_else(|_| "ephemera-vault".to_string());
                let cloud = cloud_store::CloudStore::load(&gcs_key_path, gcs_bucket);

                let app_state = Arc::new(AppState::new(vault_path, db, cloud));
                handle.manage(app_state.clone());

                metrics::spawn_sampler(handle.clone(), app_state.clone());
                state::spawn_slow_metrics_refresher(app_state);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ram::upload_to_ram,
            commands::ram::list_ram,
            commands::ram::delete_from_ram,
            commands::ram::flush_ram,
            commands::disk::persist_to_disk,
            commands::disk::list_disk,
            commands::disk::rescan_vault,
            commands::disk::delete_from_disk,
            commands::disk::reveal_vault,
            commands::db::save_to_db,
            commands::db::list_db,
            commands::db::delete_from_db,
            commands::db::get_db_status,
            commands::cloud::save_to_cloud,
            commands::cloud::list_cloud,
            commands::cloud::delete_from_cloud,
            commands::cloud::get_cloud_status,
            commands::stream::stream_upload_to_disk,
            commands::config::get_config,
            commands::config::set_vault_path,
            commands::config::get_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

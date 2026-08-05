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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Manager;

/// Walks upward from `start`, returning the first ancestor directory containing
/// `filename`. Anchoring the search on the executable's own location (rather than
/// the process's current working directory) means `.env` and `gcs-key.json` are
/// found the same way whether the app is launched via `cargo tauri dev` (exe under
/// `src-tauri/target/debug`) or as a standalone release binary run from anywhere.
fn find_upwards(start: &Path, filename: &str) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(filename);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));

    let env_path = exe_dir.as_deref().and_then(|d| find_upwards(d, ".env"));
    match &env_path {
        Some(path) => {
            let _ = dotenvy::from_path(path);
        }
        None => {
            let _ = dotenvy::dotenv();
        }
    }
    let config_dir = env_path
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .or(exe_dir)
        .unwrap_or_else(|| PathBuf::from("."));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(move |app| {
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

                let gcs_key_path = std::env::var("GCS_KEY_PATH")
                    .unwrap_or_else(|_| "gcs-key.json".to_string());
                // GCS_KEY_PATH (whether from .env or its default) is conventionally a bare
                // filename meant to sit next to .env — resolve it against config_dir rather
                // than the process's current working directory, which varies by launch method.
                let gcs_key_path = if Path::new(&gcs_key_path).is_absolute() {
                    gcs_key_path
                } else {
                    config_dir
                        .join(&gcs_key_path)
                        .to_string_lossy()
                        .into_owned()
                };
                let gcs_bucket =
                    std::env::var("GCS_BUCKET").unwrap_or_else(|_| "ephemera-vault".to_string());
                let cloud = cloud_store::CloudStore::load(&gcs_key_path, gcs_bucket);
                #[cfg(debug_assertions)]
                eprintln!(
                    "DEBUG cwd={:?} gcs_key_path={:?} connected={} reason={:?}",
                    std::env::current_dir(),
                    gcs_key_path,
                    cloud.is_connected(),
                    cloud.offline_reason()
                );

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

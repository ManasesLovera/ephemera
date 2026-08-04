use crate::error::AppError;
use crate::state::AppState;
use crate::types::{Config, Metrics};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_config(state: State<'_, Arc<AppState>>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_vault_path(state: State<'_, Arc<AppState>>, path: String) -> Result<Config, AppError> {
    let new_root = std::path::PathBuf::from(&path);
    std::fs::create_dir_all(&new_root)?;
    {
        let mut vault = state.vault.lock().unwrap();
        *vault = crate::vault::Vault::open(new_root)?;
    }
    let mut cfg = state.config.lock().unwrap();
    cfg.vault_path = path;
    Ok(cfg.clone())
}

#[tauri::command]
pub fn get_metrics(state: State<'_, Arc<AppState>>) -> Metrics {
    let ram_bytes = state.ram.lock().unwrap().total_bytes();
    let disk_bytes = state.vault.lock().unwrap().total_bytes();
    let (db_bytes, db_physical) = state.db_usage_cached();
    let cloud_bytes = state.cloud_usage_cached();
    Metrics {
        ts: chrono::Utc::now().timestamp_millis(),
        ram_store_bytes: ram_bytes,
        ram_cap: crate::types::MAX_RAM_BYTES,
        disk_store_bytes: disk_bytes,
        disk_cap: crate::types::MAX_DISK_BYTES,
        db_store_bytes: db_bytes,
        db_cap: crate::types::MAX_DB_BYTES,
        db_physical_bytes: db_physical,
        cloud_store_bytes: cloud_bytes,
        cloud_cap: crate::types::MAX_CLOUD_BYTES,
        process_rss_bytes: crate::metrics::sample_rss_now(),
        process_count: 1,
    }
}

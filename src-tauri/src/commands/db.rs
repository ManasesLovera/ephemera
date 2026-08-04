use crate::error::AppError;
use crate::state::AppState;
use crate::types::{DbFile, DbStatus, FileMeta, Origin, MAX_DB_BYTES};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_to_db(
    state: State<'_, Arc<AppState>>,
    id: String,
    source: String,
) -> Result<DbFile, AppError> {
    let (meta, bytes): (FileMeta, Vec<u8>) = match source.as_str() {
        "ram" => {
            let ram = state.ram.lock().unwrap();
            let f = ram
                .get(&id)
                .ok_or_else(|| AppError::NotFound { id: id.clone() })?;
            (f.meta.clone(), f.bytes.to_vec())
        }
        "disk" => {
            let path = state.vault.lock().unwrap().get_path(&id)?;
            let bytes = std::fs::read(&path)?;
            let vault = state.vault.lock().unwrap();
            let meta = vault
                .list()
                .into_iter()
                .find(|f| f.meta.id == id)
                .map(|f| f.meta)
                .ok_or_else(|| AppError::NotFound { id: id.clone() })?;
            (meta, bytes)
        }
        _ => {
            return Err(AppError::BadRequest {
                message: "source must be 'ram' or 'disk'".into(),
            })
        }
    };
    let origin = if source == "ram" {
        Origin::Ram
    } else {
        Origin::Disk
    };
    state.db.insert(&meta, &bytes, origin).await
}

#[tauri::command]
pub async fn list_db(state: State<'_, Arc<AppState>>) -> Result<Vec<DbFile>, AppError> {
    state.db.list().await
}

#[tauri::command]
pub async fn delete_from_db(state: State<'_, Arc<AppState>>, id: String) -> Result<(), AppError> {
    state.db.remove(&id).await
}

#[tauri::command]
pub async fn get_db_status(state: State<'_, Arc<AppState>>) -> Result<DbStatus, ()> {
    let connected = state.db.is_connected();
    let logical = state.db.logical_bytes().await.unwrap_or(0);
    let physical = state.db.physical_bytes().await.unwrap_or(0);
    Ok(DbStatus {
        connected,
        logical_bytes: logical,
        physical_bytes: physical,
        cap: MAX_DB_BYTES,
        message: state.db.offline_reason(),
    })
}

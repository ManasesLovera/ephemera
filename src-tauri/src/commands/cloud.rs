use crate::error::AppError;
use crate::state::AppState;
use crate::types::{CloudFile, CloudStatus, FileMeta, MAX_CLOUD_BYTES};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_to_cloud(
    state: State<'_, Arc<AppState>>,
    id: String,
    source: String,
) -> Result<CloudFile, AppError> {
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
    state.cloud.upload(&meta, bytes).await
}

#[tauri::command]
pub async fn list_cloud(state: State<'_, Arc<AppState>>) -> Result<Vec<CloudFile>, AppError> {
    state.cloud.list().await
}

#[tauri::command]
pub async fn delete_from_cloud(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), AppError> {
    state.cloud.remove(&id).await
}

#[tauri::command]
pub async fn get_cloud_status(state: State<'_, Arc<AppState>>) -> Result<CloudStatus, ()> {
    let connected = state.cloud.is_connected();
    let bytes_used = state.cloud.bytes_used().await.unwrap_or(0);
    Ok(CloudStatus {
        connected,
        bytes_used,
        cap: MAX_CLOUD_BYTES,
        bucket: connected.then(|| state.cloud.bucket().to_string()),
        message: state.cloud.offline_reason(),
    })
}

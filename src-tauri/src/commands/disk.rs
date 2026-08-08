use ephemera_core::error::AppError;
use ephemera_core::state::AppState;
use ephemera_core::types::{DiskFile, TransferResult};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn persist_to_disk(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<TransferResult, AppError> {
    ephemera_core::persist_to_disk(&state, &id)
}

#[tauri::command]
pub fn list_disk(state: State<'_, Arc<AppState>>) -> Vec<DiskFile> {
    ephemera_core::list_disk(&state)
}

#[tauri::command]
pub fn rescan_vault(state: State<'_, Arc<AppState>>) -> Result<Vec<DiskFile>, AppError> {
    ephemera_core::rescan_vault(&state)
}

#[tauri::command]
pub fn delete_from_disk(state: State<'_, Arc<AppState>>, id: String) -> Result<(), AppError> {
    ephemera_core::delete_from_disk(&state, &id)
}

#[tauri::command]
pub fn reveal_vault(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    let root = ephemera_core::get_vault_path(&state);
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(root)
        .map_err(|e| AppError::Io {
            message: e.to_string(),
        })?;
    Ok(())
}

use ephemera_core::error::AppError;
use ephemera_core::state::AppState;
use ephemera_core::types::FileMeta;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn upload_to_ram(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<FileMeta, AppError> {
    ephemera_core::upload_to_ram(&state, &path).await
}

#[tauri::command]
pub fn list_ram(state: State<'_, Arc<AppState>>) -> Vec<FileMeta> {
    ephemera_core::list_ram(&state)
}

#[tauri::command]
pub fn delete_from_ram(state: State<'_, Arc<AppState>>, id: String) -> Result<(), AppError> {
    ephemera_core::delete_from_ram(&state, &id)
}

#[tauri::command]
pub fn flush_ram(state: State<'_, Arc<AppState>>) {
    ephemera_core::flush_ram(&state);
}

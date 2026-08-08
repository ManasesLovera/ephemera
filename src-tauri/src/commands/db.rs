use ephemera_core::error::AppError;
use ephemera_core::state::AppState;
use ephemera_core::types::{DbFile, DbStatus};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_to_db(
    state: State<'_, Arc<AppState>>,
    id: String,
    source: String,
) -> Result<DbFile, AppError> {
    ephemera_core::save_to_db(&state, &id, &source).await
}

#[tauri::command]
pub async fn list_db(state: State<'_, Arc<AppState>>) -> Result<Vec<DbFile>, AppError> {
    ephemera_core::list_db(&state).await
}

#[tauri::command]
pub async fn delete_from_db(state: State<'_, Arc<AppState>>, id: String) -> Result<(), AppError> {
    ephemera_core::delete_from_db(&state, &id).await
}

#[tauri::command]
pub async fn get_db_status(state: State<'_, Arc<AppState>>) -> Result<DbStatus, ()> {
    ephemera_core::get_db_status(&state).await
}

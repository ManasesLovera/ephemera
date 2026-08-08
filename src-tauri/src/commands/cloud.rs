use ephemera_core::error::AppError;
use ephemera_core::state::AppState;
use ephemera_core::types::{CloudFile, CloudStatus};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_to_cloud(
    state: State<'_, Arc<AppState>>,
    id: String,
    source: String,
) -> Result<CloudFile, AppError> {
    ephemera_core::save_to_cloud(&state, &id, &source).await
}

#[tauri::command]
pub async fn list_cloud(state: State<'_, Arc<AppState>>) -> Result<Vec<CloudFile>, AppError> {
    ephemera_core::list_cloud(&state).await
}

#[tauri::command]
pub async fn delete_from_cloud(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), AppError> {
    ephemera_core::delete_from_cloud(&state, &id).await
}

#[tauri::command]
pub async fn get_cloud_status(state: State<'_, Arc<AppState>>) -> Result<CloudStatus, ()> {
    ephemera_core::get_cloud_status(&state).await
}

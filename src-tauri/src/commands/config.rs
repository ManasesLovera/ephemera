use ephemera_core::error::AppError;
use ephemera_core::state::AppState;
use ephemera_core::types::{Config, Metrics};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_config(state: State<'_, Arc<AppState>>) -> Config {
    ephemera_core::get_config(&state)
}

#[tauri::command]
pub fn set_vault_path(state: State<'_, Arc<AppState>>, path: String) -> Result<Config, AppError> {
    ephemera_core::set_vault_path(&state, &path)
}

#[tauri::command]
pub fn get_metrics(state: State<'_, Arc<AppState>>) -> Metrics {
    ephemera_core::get_metrics(&state)
}

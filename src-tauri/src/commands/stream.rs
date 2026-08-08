use ephemera_core::error::AppError;
use ephemera_core::state::AppState;
use ephemera_core::types::{StreamReport, TransferProgress};
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

#[tauri::command]
pub async fn stream_upload_to_disk(
    state: State<'_, Arc<AppState>>,
    path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<StreamReport, AppError> {
    ephemera_core::stream_upload_to_disk(&state, &path, move |p| {
        let _ = on_progress.send(p);
    })
}

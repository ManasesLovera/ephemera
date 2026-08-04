use crate::error::AppError;
use crate::state::AppState;
use crate::types::{StreamReport, TransferProgress};
use crate::vault::sanitize_filename;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

/// Streams a file directly from `path` into the vault, never buffering the whole file
/// in RAM. See docs/07-streaming.md for the report this produces.
#[tauri::command]
pub async fn stream_upload_to_disk(
    state: State<'_, Arc<AppState>>,
    path: String,
    on_progress: Channel<TransferProgress>,
) -> Result<StreamReport, AppError> {
    let source = std::path::PathBuf::from(&path);
    let size = std::fs::metadata(&source)?.len();
    let name = sanitize_filename(source.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed"))?;

    {
        let vault = state.vault.lock().unwrap();
        vault.assert_room_for(size)?;
    }
    let dest = {
        let vault = state.vault.lock().unwrap();
        vault.write_new_path(&name)?
    };

    let file_id = uuid::Uuid::new_v4().to_string();
    let report = crate::stream::stream_copy(&source, &dest, file_id, &on_progress)?;

    let final_name = dest.file_name().unwrap().to_string_lossy().to_string();
    let meta = crate::types::FileMeta {
        id: uuid::Uuid::new_v4().to_string(),
        name: final_name,
        size,
        mime: mime_guess::from_path(&name).first_or_octet_stream().to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
        origin: crate::types::Origin::Stream,
    };
    state.vault.lock().unwrap().register(meta);

    Ok(report)
}

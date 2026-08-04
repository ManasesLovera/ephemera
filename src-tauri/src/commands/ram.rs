use crate::error::AppError;
use crate::state::AppState;
use crate::types::{FileMeta, Origin};
use crate::vault::sanitize_filename;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Reads the file at `path` and holds its bytes entirely in the RAM store. Path-based
/// (not raw IPC bytes) so the file never round-trips through the webview — see
/// docs/02-architecture.md "Getting file bytes across the IPC boundary".
#[tauri::command]
pub async fn upload_to_ram(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<FileMeta, AppError> {
    let p = std::path::Path::new(&path);
    let name = sanitize_filename(p.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed"))?;
    let bytes = std::fs::read(p)?;
    let mime = mime_guess::from_path(&name)
        .first_or_octet_stream()
        .to_string();
    let meta = FileMeta {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        size: bytes.len() as u64,
        mime,
        created_at: now_millis(),
        origin: Origin::Upload,
    };
    state
        .ram
        .lock()
        .unwrap()
        .insert(meta.clone(), Arc::from(bytes.into_boxed_slice()))?;
    Ok(meta)
}

#[tauri::command]
pub fn list_ram(state: State<'_, Arc<AppState>>) -> Vec<FileMeta> {
    state.ram.lock().unwrap().list()
}

#[tauri::command]
pub fn delete_from_ram(state: State<'_, Arc<AppState>>, id: String) -> Result<(), AppError> {
    state.ram.lock().unwrap().remove(&id)?;
    Ok(())
}

#[tauri::command]
pub fn flush_ram(state: State<'_, Arc<AppState>>) {
    state.ram.lock().unwrap().flush();
}

use crate::error::AppError;
use crate::state::AppState;
use crate::types::{DiskFile, FileMeta, Origin, TransferResult};
use std::sync::Arc;
use std::time::Instant;
use tauri::State;

/// RAM -> disk. Takes an Arc clone of the bytes (pointer copy, not a duplicate
/// allocation) and releases the RAM lock before the write, so the app stays
/// responsive and the "a reference is not a copy" teaching point holds for real.
#[tauri::command]
pub fn persist_to_disk(state: State<'_, Arc<AppState>>, id: String) -> Result<TransferResult, AppError> {
    let (meta, bytes) = {
        let ram = state.ram.lock().unwrap();
        let file = ram.get(&id).ok_or_else(|| AppError::NotFound { id: id.clone() })?;
        (file.meta.clone(), file.bytes.clone())
    };

    {
        let vault = state.vault.lock().unwrap();
        vault.assert_room_for(meta.size)?;
    }

    let started = Instant::now();
    let dest = {
        let vault = state.vault.lock().unwrap();
        vault.write_new_path(&meta.name)?
    };
    std::fs::write(&dest, &bytes[..])?;
    let f = std::fs::File::open(&dest)?;
    f.sync_all()?;

    let final_name = dest.file_name().unwrap().to_string_lossy().to_string();
    let disk_meta = FileMeta { name: final_name, origin: Origin::Ram, ..meta.clone() };
    state.vault.lock().unwrap().register(disk_meta);

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let throughput_mb_s = if elapsed_ms > 0 {
        (meta.size as f64 / 1_048_576.0) / (elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };
    Ok(TransferResult { id, bytes: meta.size, elapsed_ms, throughput_mb_s })
}

#[tauri::command]
pub fn list_disk(state: State<'_, Arc<AppState>>) -> Vec<DiskFile> {
    state.vault.lock().unwrap().list()
}

#[tauri::command]
pub fn rescan_vault(state: State<'_, Arc<AppState>>) -> Result<Vec<DiskFile>, AppError> {
    let mut vault = state.vault.lock().unwrap();
    vault.rescan()?;
    Ok(vault.list())
}

#[tauri::command]
pub fn delete_from_disk(state: State<'_, Arc<AppState>>, id: String) -> Result<(), AppError> {
    state.vault.lock().unwrap().remove(&id)
}

#[tauri::command]
pub fn reveal_vault(state: State<'_, Arc<AppState>>, app: tauri::AppHandle) -> Result<(), AppError> {
    let root = state.vault.lock().unwrap().root().to_path_buf();
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(root)
        .map_err(|e| AppError::Io { message: e.to_string() })?;
    Ok(())
}

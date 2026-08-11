pub mod cloud_store;
pub mod config_file;
pub mod db_store;
pub mod error;
pub mod metrics;
pub mod ram_store;
pub mod state;
pub mod stream;
pub mod types;
pub mod vault;

use error::AppError;
use state::AppState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use types::*;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn find_upwards(start: &Path, filename: &str) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(filename);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

// ==================== RAM STORE ====================

pub async fn upload_to_ram(state: &AppState, path: &str) -> Result<FileMeta, AppError> {
    let p = Path::new(path);
    let name =
        vault::sanitize_filename(p.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed"))?;
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

pub fn list_ram(state: &AppState) -> Vec<FileMeta> {
    state.ram.lock().unwrap().list()
}

pub fn delete_from_ram(state: &AppState, id: &str) -> Result<(), AppError> {
    state.ram.lock().unwrap().remove(id)?;
    Ok(())
}

pub fn flush_ram(state: &AppState) {
    state.ram.lock().unwrap().flush();
}

// ==================== VAULT DISK STORE ====================

pub fn persist_to_disk(state: &AppState, id: &str) -> Result<TransferResult, AppError> {
    let (meta, bytes) = {
        let ram = state.ram.lock().unwrap();
        let file = ram
            .get(id)
            .ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
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
    let disk_meta = FileMeta {
        name: final_name,
        origin: Origin::Ram,
        ..meta.clone()
    };
    state.vault.lock().unwrap().register(disk_meta);

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let throughput_mb_s = if elapsed_ms > 0 {
        (meta.size as f64 / 1_048_576.0) / (elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };
    Ok(TransferResult {
        id: id.to_string(),
        bytes: meta.size,
        elapsed_ms,
        throughput_mb_s,
    })
}

pub fn list_disk(state: &AppState) -> Vec<DiskFile> {
    state.vault.lock().unwrap().list()
}

pub fn rescan_vault(state: &AppState) -> Result<Vec<DiskFile>, AppError> {
    let mut vault = state.vault.lock().unwrap();
    vault.rescan()?;
    Ok(vault.list())
}

pub fn delete_from_disk(state: &AppState, id: &str) -> Result<(), AppError> {
    state.vault.lock().unwrap().remove(id)
}

pub fn get_vault_path(state: &AppState) -> PathBuf {
    state.vault.lock().unwrap().root().to_path_buf()
}

// ==================== POSTGRES DB STORE ====================

pub async fn save_to_db(state: &AppState, id: &str, source: &str) -> Result<DbFile, AppError> {
    let (meta, bytes): (FileMeta, Vec<u8>) = match source {
        "ram" => {
            let ram = state.ram.lock().unwrap();
            let f = ram
                .get(id)
                .ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
            (f.meta.clone(), f.bytes.to_vec())
        }
        "disk" => {
            let path = state.vault.lock().unwrap().get_path(id)?;
            let bytes = std::fs::read(&path)?;
            let vault = state.vault.lock().unwrap();
            let meta = vault
                .list()
                .into_iter()
                .find(|f| f.meta.id == id)
                .map(|f| f.meta)
                .ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
            (meta, bytes)
        }
        _ => {
            return Err(AppError::BadRequest {
                message: "source must be 'ram' or 'disk'".into(),
            })
        }
    };
    let origin = if source == "ram" {
        Origin::Ram
    } else {
        Origin::Disk
    };
    state.db.insert(&meta, &bytes, origin).await
}

pub async fn list_db(state: &AppState) -> Result<Vec<DbFile>, AppError> {
    state.db.list().await
}

pub async fn delete_from_db(state: &AppState, id: &str) -> Result<(), AppError> {
    state.db.remove(&id.to_string()).await
}

pub async fn get_db_status(state: &AppState) -> Result<DbStatus, ()> {
    let connected = state.db.is_connected();
    let logical = state.db.logical_bytes().await.unwrap_or(0);
    let physical = state.db.physical_bytes().await.unwrap_or(0);
    Ok(DbStatus {
        connected,
        logical_bytes: logical,
        physical_bytes: physical,
        cap: MAX_DB_BYTES,
        message: state.db.offline_reason(),
    })
}

// ==================== GCS CLOUD STORE ====================

pub async fn save_to_cloud(
    state: &AppState,
    id: &str,
    source: &str,
) -> Result<CloudFile, AppError> {
    let (meta, bytes): (FileMeta, Vec<u8>) = match source {
        "ram" => {
            let ram = state.ram.lock().unwrap();
            let f = ram
                .get(id)
                .ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
            (f.meta.clone(), f.bytes.to_vec())
        }
        "disk" => {
            let path = state.vault.lock().unwrap().get_path(id)?;
            let bytes = std::fs::read(&path)?;
            let vault = state.vault.lock().unwrap();
            let meta = vault
                .list()
                .into_iter()
                .find(|f| f.meta.id == id)
                .map(|f| f.meta)
                .ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
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

pub async fn list_cloud(state: &AppState) -> Result<Vec<CloudFile>, AppError> {
    state.cloud.list().await
}

pub async fn delete_from_cloud(state: &AppState, id: &str) -> Result<(), AppError> {
    state.cloud.remove(&id.to_string()).await
}

pub async fn get_cloud_status(state: &AppState) -> Result<CloudStatus, ()> {
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

// ==================== STREAMING ====================

pub fn stream_upload_to_disk<F>(
    state: &AppState,
    path: &str,
    on_progress: F,
) -> Result<StreamReport, AppError>
where
    F: FnMut(TransferProgress),
{
    let source = PathBuf::from(path);
    let size = std::fs::metadata(&source)?.len();
    let name = vault::sanitize_filename(
        source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed"),
    )?;

    {
        let vault = state.vault.lock().unwrap();
        vault.assert_room_for(size)?;
    }
    let dest = {
        let vault = state.vault.lock().unwrap();
        vault.write_new_path(&name)?
    };

    let file_id = uuid::Uuid::new_v4().to_string();
    let report = stream::stream_copy(&source, &dest, file_id, on_progress)?;

    let final_name = dest.file_name().unwrap().to_string_lossy().to_string();
    let meta = FileMeta {
        id: uuid::Uuid::new_v4().to_string(),
        name: final_name,
        size,
        mime: mime_guess::from_path(&name)
            .first_or_octet_stream()
            .to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
        origin: Origin::Stream,
    };
    state.vault.lock().unwrap().register(meta);

    Ok(report)
}

// ==================== CONFIG & METRICS ====================

pub fn get_config(state: &AppState) -> Config {
    state.config.lock().unwrap().clone()
}

pub fn set_vault_path(state: &AppState, path: &str) -> Result<Config, AppError> {
    let new_root = PathBuf::from(path);
    std::fs::create_dir_all(&new_root)?;
    {
        let mut vault = state.vault.lock().unwrap();
        *vault = vault::Vault::open(new_root)?;
    }
    let cfg = {
        let mut cfg = state.config.lock().unwrap();
        cfg.vault_path = path.to_string();
        cfg.clone()
    };
    // Persist the choice so the next launch starts on this vault
    // (docs/01-requirements.md, "Configuration" MUST). Best-effort: the
    // in-memory switch already succeeded, so a failed config write must not
    // turn into a failed operation — the app keeps working exactly as it did
    // before persistence existed.
    if let Err(e) = config_file::save_vault_path(path) {
        eprintln!("ephemera: vault path switched but could not be persisted: {e}");
    }
    Ok(cfg)
}

pub fn get_metrics(state: &AppState) -> Metrics {
    let ram_bytes = state.ram.lock().unwrap().total_bytes();
    let disk_bytes = state.vault.lock().unwrap().total_bytes();
    let (db_bytes, db_physical) = state.db_usage_cached();
    let cloud_bytes = state.cloud_usage_cached();
    Metrics {
        ts: chrono::Utc::now().timestamp_millis(),
        ram_store_bytes: ram_bytes,
        ram_cap: MAX_RAM_BYTES,
        disk_store_bytes: disk_bytes,
        disk_cap: MAX_DISK_BYTES,
        db_store_bytes: db_bytes,
        db_cap: MAX_DB_BYTES,
        db_physical_bytes: db_physical,
        cloud_store_bytes: cloud_bytes,
        cloud_cap: MAX_CLOUD_BYTES,
        process_rss_bytes: metrics::sample_rss_now(),
        process_count: metrics::sample_process_count_now(),
    }
}

pub fn spawn_sampler<F>(state: Arc<AppState>, callback: F) -> std::thread::JoinHandle<()>
where
    F: Fn(Metrics) + Send + 'static,
{
    metrics::spawn_sampler(state, callback)
}

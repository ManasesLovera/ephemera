use serde::{Deserialize, Serialize};

pub type FileId = String;

pub const MAX_RAM_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_DISK_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_SINGLE_FILE: u64 = MAX_RAM_BYTES;
pub const MAX_DB_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_CLOUD_BYTES: u64 = 100 * 1024 * 1024;
pub const STREAM_CHUNK_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Upload,
    Stream,
    Ram,
    Disk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub id: FileId,
    pub name: String,
    pub size: u64,
    pub mime: String,
    pub created_at: i64,
    pub origin: Origin,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskFile {
    pub meta: FileMeta,
    pub persisted_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbFile {
    pub meta: FileMeta,
    pub saved_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudFile {
    pub meta: FileMeta,
    pub saved_at: i64,
    pub object_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    RamToDisk,
    StreamToDisk,
    RamToDb,
    DiskToDb,
    RamToCloud,
    DiskToCloud,
    UploadToRam,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferProgress {
    pub id: FileId,
    pub direction: Direction,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferResult {
    pub id: FileId,
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub throughput_mb_s: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamReport {
    pub file_name: String,
    pub bytes_total: u64,
    pub chunk_size: u64,
    pub elapsed_ms: u64,
    pub throughput_mb_s: f64,
    pub rss_baseline_bytes: u64,
    pub rss_peak_bytes: u64,
    pub rss_avg_bytes: u64,
    pub rss_peak_delta_bytes: i64,
    pub buffered_equivalent_peak_bytes: u64,
    pub max_concurrent_streaming: u64,
    pub max_concurrent_buffered: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub ts: i64,
    pub ram_store_bytes: u64,
    pub ram_cap: u64,
    pub disk_store_bytes: u64,
    pub disk_cap: u64,
    pub db_store_bytes: u64,
    pub db_cap: u64,
    pub db_physical_bytes: u64,
    pub cloud_store_bytes: u64,
    pub cloud_cap: u64,
    pub process_rss_bytes: u64,
    pub process_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub vault_path: String,
    pub throttle_ms_per_chunk: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DbStatus {
    pub connected: bool,
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub cap: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudStatus {
    pub connected: bool,
    pub bytes_used: u64,
    pub cap: u64,
    pub bucket: Option<String>,
    pub message: Option<String>,
}

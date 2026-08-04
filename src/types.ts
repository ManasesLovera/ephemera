export type FileId = string;

export const MAX_RAM_BYTES = 10 * 1024 * 1024;
export const MAX_DISK_BYTES = 20 * 1024 * 1024;
export const MAX_DB_BYTES = 100 * 1024 * 1024;
export const MAX_CLOUD_BYTES = 100 * 1024 * 1024;
export const STREAM_CHUNK_BYTES = 256 * 1024;

export type Origin = "upload" | "stream" | "ram" | "disk";

export interface FileMeta {
  id: FileId;
  name: string;
  size: number;
  mime: string;
  created_at: number;
  origin: Origin;
}

export interface DiskFile {
  meta: FileMeta;
  persisted_at: number;
}

export interface DbFile {
  meta: FileMeta;
  saved_at: number;
}

export interface CloudFile {
  meta: FileMeta;
  saved_at: number;
  object_name: string;
}

export type Direction =
  | "ram_to_disk"
  | "stream_to_disk"
  | "ram_to_db"
  | "disk_to_db"
  | "ram_to_cloud"
  | "disk_to_cloud"
  | "upload_to_ram";

export interface TransferProgress {
  id: FileId;
  direction: Direction;
  bytes_done: number;
  bytes_total: number;
}

export interface TransferResult {
  id: FileId;
  bytes: number;
  elapsed_ms: number;
  throughput_mb_s: number;
}

export interface StreamReport {
  file_name: string;
  bytes_total: number;
  chunk_size: number;
  elapsed_ms: number;
  throughput_mb_s: number;
  rss_baseline_bytes: number;
  rss_peak_bytes: number;
  rss_avg_bytes: number;
  rss_peak_delta_bytes: number;
  buffered_equivalent_peak_bytes: number;
  max_concurrent_streaming: number;
  max_concurrent_buffered: number;
}

export interface Metrics {
  ts: number;
  ram_store_bytes: number;
  ram_cap: number;
  disk_store_bytes: number;
  disk_cap: number;
  db_store_bytes: number;
  db_cap: number;
  db_physical_bytes: number;
  cloud_store_bytes: number;
  cloud_cap: number;
  process_rss_bytes: number;
  process_count: number;
}

export interface Config {
  vault_path: string;
  throttle_ms_per_chunk: number;
}

export interface DbStatus {
  connected: boolean;
  logical_bytes: number;
  physical_bytes: number;
  cap: number;
  message: string | null;
}

export interface CloudStatus {
  connected: boolean;
  bytes_used: number;
  cap: number;
  bucket: string | null;
  message: string | null;
}

export interface AppErrorPayload {
  kind: string;
  [key: string]: unknown;
}

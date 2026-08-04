import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  CloudFile,
  CloudStatus,
  Config,
  DbFile,
  DbStatus,
  DiskFile,
  FileMeta,
  Metrics,
  StreamReport,
  TransferProgress,
  TransferResult,
} from "../types";

export const ipc = {
  uploadToRam: (path: string) => invoke<FileMeta>("upload_to_ram", { path }),
  listRam: () => invoke<FileMeta[]>("list_ram"),
  deleteFromRam: (id: string) => invoke<void>("delete_from_ram", { id }),
  flushRam: () => invoke<void>("flush_ram"),

  persistToDisk: (id: string) => invoke<TransferResult>("persist_to_disk", { id }),
  listDisk: () => invoke<DiskFile[]>("list_disk"),
  rescanVault: () => invoke<DiskFile[]>("rescan_vault"),
  deleteFromDisk: (id: string) => invoke<void>("delete_from_disk", { id }),
  revealVault: () => invoke<void>("reveal_vault"),

  saveToDb: (id: string, source: "ram" | "disk") => invoke<DbFile>("save_to_db", { id, source }),
  listDb: () => invoke<DbFile[]>("list_db"),
  deleteFromDb: (id: string) => invoke<void>("delete_from_db", { id }),
  getDbStatus: () => invoke<DbStatus>("get_db_status"),

  saveToCloud: (id: string, source: "ram" | "disk") => invoke<CloudFile>("save_to_cloud", { id, source }),
  listCloud: () => invoke<CloudFile[]>("list_cloud"),
  deleteFromCloud: (id: string) => invoke<void>("delete_from_cloud", { id }),
  getCloudStatus: () => invoke<CloudStatus>("get_cloud_status"),

  streamUploadToDisk: (path: string, onProgress: (p: TransferProgress) => void) => {
    const channel = new Channel<TransferProgress>();
    channel.onmessage = onProgress;
    return invoke<StreamReport>("stream_upload_to_disk", { path, onProgress: channel });
  },

  getConfig: () => invoke<Config>("get_config"),
  setVaultPath: (path: string) => invoke<Config>("set_vault_path", { path }),
  getMetrics: () => invoke<Metrics>("get_metrics"),
};

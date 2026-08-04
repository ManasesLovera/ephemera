import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { ipc } from "../lib/ipc";
import type {
  CloudFile,
  CloudStatus,
  DbFile,
  DbStatus,
  DiskFile,
  FileMeta,
  Metrics,
} from "../types";

interface MetricPoint {
  ts: number;
  ram: number;
  rss: number;
}

interface AppStore {
  ramFiles: FileMeta[];
  diskFiles: DiskFile[];
  dbFiles: DbFile[];
  cloudFiles: CloudFile[];
  metrics: Metrics | null;
  history: MetricPoint[];
  dbStatus: DbStatus | null;
  cloudStatus: CloudStatus | null;
  vaultPath: string;
  refreshRam: () => Promise<void>;
  refreshDisk: () => Promise<void>;
  refreshDb: () => Promise<void>;
  refreshCloud: () => Promise<void>;
  refreshAll: () => Promise<void>;
  init: () => Promise<void>;
}

const HISTORY_LIMIT = 240; // 60s at 4Hz

export const useAppStore = create<AppStore>((set, get) => ({
  ramFiles: [],
  diskFiles: [],
  dbFiles: [],
  cloudFiles: [],
  metrics: null,
  history: [],
  dbStatus: null,
  cloudStatus: null,
  vaultPath: "",

  refreshRam: async () => set({ ramFiles: await ipc.listRam() }),
  refreshDisk: async () => set({ diskFiles: await ipc.listDisk() }),
  refreshDb: async () => {
    try {
      set({ dbFiles: await ipc.listDb(), dbStatus: await ipc.getDbStatus() });
    } catch {
      set({ dbStatus: await ipc.getDbStatus().catch(() => null) });
    }
  },
  refreshCloud: async () => {
    try {
      set({ cloudFiles: await ipc.listCloud(), cloudStatus: await ipc.getCloudStatus() });
    } catch {
      set({ cloudStatus: await ipc.getCloudStatus().catch(() => null) });
    }
  },
  refreshAll: async () => {
    await Promise.all([get().refreshRam(), get().refreshDisk(), get().refreshDb(), get().refreshCloud()]);
  },

  init: async () => {
    const cfg = await ipc.getConfig();
    set({ vaultPath: cfg.vault_path });
    await get().refreshAll();

    await listen<Metrics>("metrics://tick", (event) => {
      const m = event.payload;
      set((s) => {
        const point: MetricPoint = { ts: m.ts, ram: m.ram_store_bytes, rss: m.process_rss_bytes };
        const history = [...s.history, point].slice(-HISTORY_LIMIT);
        return { metrics: m, history };
      });
    });
  },
}));

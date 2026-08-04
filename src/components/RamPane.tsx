import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useAppStore } from "../store/useAppStore";
import { ipc } from "../lib/ipc";
import { Meter } from "./Meter";
import { FileCard } from "./FileCard";
import { MAX_RAM_BYTES } from "../types";
import type { StreamReport } from "../types";

export function RamPane({ onError, onStreamReport }: { onError: (m: string) => void; onStreamReport: (r: StreamReport) => void }) {
  const { ramFiles, refreshRam, refreshDisk, refreshDb, refreshCloud } = useAppStore();
  const [dragOver, setDragOver] = useState(false);
  const dropRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          setDragOver(true);
        } else if (event.payload.type === "drop") {
          setDragOver(false);
          const paths = event.payload.paths;
          paths.forEach((p) => uploadPath(p));
        } else {
          setDragOver(false);
        }
      })
      .then((fn) => (unlisten = fn));
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const uploadPath = async (path: string) => {
    try {
      await ipc.uploadToRam(path);
      await refreshRam();
    } catch (e) {
      onError(describeError(e));
    }
  };

  const browse = async () => {
    const selected = await open({ multiple: true });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    for (const p of paths) await uploadPath(p);
  };

  const streamToDisk = async () => {
    const selected = await open({ multiple: false });
    if (!selected || Array.isArray(selected)) return;
    try {
      const report = await ipc.streamUploadToDisk(selected, () => {});
      onStreamReport(report);
      await refreshDisk();
    } catch (e) {
      onError(describeError(e));
    }
  };

  const pullThePlug = async () => {
    await ipc.flushRam();
    await refreshRam();
  };

  return (
    <div className="pane">
      <h2>RAM <span className="subtitle">volatile — cleared when the app closes</span></h2>
      <Meter used={ramFiles.reduce((a, f) => a + f.size, 0)} cap={MAX_RAM_BYTES} segments={ramFiles.map((f) => ({ id: f.id, name: f.name, size: f.size }))} />
      <div
        ref={dropRef}
        className={`dropzone ${dragOver ? "drag-over" : ""}`}
      >
        {ramFiles.length === 0 ? "drop files here" : `${ramFiles.length} file(s) in RAM`}
      </div>
      <div className="file-list">
        {ramFiles.map((f) => (
          <FileCard
            key={f.id}
            meta={f}
            actions={[
              {
                label: "→ Disk",
                title: "Persist to disk",
                onClick: async () => {
                  try {
                    await ipc.persistToDisk(f.id);
                    await refreshDisk();
                  } catch (e) {
                    onError(describeError(e));
                  }
                },
              },
              {
                label: "→ DB",
                title: "Save to database",
                onClick: async () => {
                  try {
                    await ipc.saveToDb(f.id, "ram");
                    await refreshDb();
                  } catch (e) {
                    onError(describeError(e));
                  }
                },
              },
              {
                label: "→ Cloud",
                title: "Save to cloud",
                onClick: async () => {
                  try {
                    await ipc.saveToCloud(f.id, "ram");
                    await refreshCloud();
                  } catch (e) {
                    onError(describeError(e));
                  }
                },
              },
              {
                label: "✕",
                title: "Delete from RAM",
                onClick: async () => {
                  await ipc.deleteFromRam(f.id);
                  await refreshRam();
                },
              },
            ]}
          />
        ))}
      </div>
      <div className="pane-actions">
        <button className="btn primary" onClick={browse}>Upload…</button>
        <button className="btn" onClick={streamToDisk} title="Bypasses RAM entirely — see the streaming report">Stream to disk…</button>
        <button className="btn danger" onClick={pullThePlug}>Pull the plug</button>
      </div>
    </div>
  );
}

function describeError(e: unknown): string {
  const err = e as { kind?: string; needed?: number; free?: number; cap?: number; size?: number; message?: string };
  if (err?.kind === "quota_exceeded") {
    return `Quota exceeded — need ${Math.ceil((err.needed ?? 0) / 1024)} KB more, ${Math.ceil((err.free ?? 0) / 1024)} KB free of ${Math.ceil((err.cap ?? 0) / 1024 / 1024)} MB.`;
  }
  if (err?.kind === "file_too_large") {
    return `File too large: ${Math.ceil((err.size ?? 0) / 1024 / 1024)} MB, cap is ${Math.ceil((err.cap ?? 0) / 1024 / 1024)} MB. Try "Stream to disk" instead.`;
  }
  return err?.message || JSON.stringify(e);
}

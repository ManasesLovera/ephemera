import { useAppStore } from "../store/useAppStore";
import { ipc } from "../lib/ipc";
import { Meter } from "./Meter";
import { FileCard } from "./FileCard";
import { MAX_DISK_BYTES } from "../types";

export function DiskPane({ onError }: { onError: (m: string) => void }) {
  const { diskFiles, vaultPath, refreshDisk, refreshDb, refreshCloud } = useAppStore();

  return (
    <div className="pane">
      <h2>Disk <span className="subtitle">persistent — survives a restart</span></h2>
      <Meter
        used={diskFiles.reduce((a, f) => a + f.meta.size, 0)}
        cap={MAX_DISK_BYTES}
        segments={diskFiles.map((f) => ({ id: f.meta.id, name: f.meta.name, size: f.meta.size }))}
      />
      <div className="dropzone">
        {diskFiles.length === 0 ? "drag from RAM to keep" : `${diskFiles.length} file(s) on disk`}
      </div>
      <div className="file-list">
        {diskFiles.map((f) => (
          <FileCard
            key={f.meta.id}
            meta={f.meta}
            actions={[
              {
                label: "→ DB",
                title: "Save to database",
                onClick: async () => {
                  try {
                    await ipc.saveToDb(f.meta.id, "disk");
                    await refreshDb();
                  } catch (e) {
                    onError((e as { message?: string })?.message || String(e));
                  }
                },
              },
              {
                label: "→ Cloud",
                title: "Save to cloud",
                onClick: async () => {
                  try {
                    await ipc.saveToCloud(f.meta.id, "disk");
                    await refreshCloud();
                  } catch (e) {
                    onError((e as { message?: string })?.message || String(e));
                  }
                },
              },
              {
                label: "✕",
                title: "Delete from disk",
                onClick: async () => {
                  await ipc.deleteFromDisk(f.meta.id);
                  await refreshDisk();
                },
              },
            ]}
          />
        ))}
      </div>
      <div className="pane-actions">
        <button className="btn" onClick={() => ipc.revealVault()}>Open folder</button>
        <button className="btn" onClick={() => refreshDisk()}>Rescan</button>
        <span className="subtitle" style={{ alignSelf: "center" }}>{vaultPath}</span>
      </div>
    </div>
  );
}

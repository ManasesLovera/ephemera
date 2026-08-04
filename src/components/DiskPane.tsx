import { useAppStore } from "../store/useAppStore";
import { ipc } from "../lib/ipc";
import { Meter } from "./Meter";
import { FileCard } from "./FileCard";
import { MAX_DISK_BYTES } from "../types";
import { useT } from "../lib/i18n";

export function DiskPane({ onError }: { onError: (m: string) => void }) {
  const { diskFiles, vaultPath, refreshDisk, refreshDb, refreshCloud } = useAppStore();
  const t = useT();

  return (
    <div className="pane">
      <h2>{t.diskTitle} <span className="subtitle">{t.diskSubtitle}</span></h2>
      <Meter
        used={diskFiles.reduce((a, f) => a + f.meta.size, 0)}
        cap={MAX_DISK_BYTES}
        segments={diskFiles.map((f) => ({ id: f.meta.id, name: f.meta.name, size: f.meta.size }))}
      />
      <div className="dropzone">
        {diskFiles.length === 0 ? t.dropHereDisk : t.filesOnDisk(diskFiles.length)}
      </div>
      <div className="file-list">
        {diskFiles.map((f) => (
          <FileCard
            key={f.meta.id}
            meta={f.meta}
            actions={[
              {
                label: t.toDb,
                title: t.saveDbTitle,
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
                label: t.toCloud,
                title: t.saveCloudTitle,
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
                title: t.deleteDiskTitle,
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
        <button className="btn" onClick={() => ipc.revealVault()}>{t.openFolder}</button>
        <button className="btn" onClick={() => refreshDisk()}>{t.rescan}</button>
        <span className="subtitle" style={{ alignSelf: "center" }}>{vaultPath}</span>
      </div>
    </div>
  );
}

import { formatBytes, pct } from "../lib/format";
import { useT } from "../lib/i18n";
import { FileCard } from "./FileCard";
import type { FileMeta } from "../types";

interface Props {
  title: string;
  subtitle: string;
  connected: boolean;
  used: number;
  cap: number;
  physical?: number;
  offlineMessage?: string | null;
  extra?: string;
  files: FileMeta[];
  deleteTitle: string;
  onDelete: (id: string) => void;
}

export function SinkPanel({
  title,
  subtitle,
  connected,
  used,
  cap,
  physical,
  offlineMessage,
  extra,
  files,
  deleteTitle,
  onDelete,
}: Props) {
  const t = useT();
  return (
    <div className="sink-panel">
      <h3>{title} <span className="subtitle">{subtitle}</span></h3>
      {!connected ? (
        <div className="offline">{offlineMessage || "offline"}</div>
      ) : (
        <>
          <div className="meter-track">
            {used > 0 && (
              <div
                className="meter-seg"
                style={{ width: `${Math.max(1, pct(used, cap))}%`, background: "var(--s6)" }}
              />
            )}
          </div>
          <div className="meter-caption">
            <span>{formatBytes(used)}</span>
            <span>of {formatBytes(cap)}</span>
          </div>
          {physical !== undefined && <div className="physical-note">{t.physicalNote(formatBytes(physical))}</div>}
          {extra && <div className="physical-note">{extra}</div>}
          <div className="file-list" style={{ marginTop: 10 }}>
            {files.length === 0 ? (
              <div className="physical-note">{t.noFilesYet}</div>
            ) : (
              files.map((f) => (
                <FileCard
                  key={f.id}
                  meta={f}
                  actions={[{ label: "✕", title: deleteTitle, onClick: () => onDelete(f.id) }]}
                />
              ))
            )}
          </div>
        </>
      )}
    </div>
  );
}

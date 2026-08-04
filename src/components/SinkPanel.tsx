import { formatBytes, pct } from "../lib/format";
import { useT } from "../lib/i18n";

interface Props {
  title: string;
  subtitle: string;
  connected: boolean;
  used: number;
  cap: number;
  physical?: number;
  offlineMessage?: string | null;
  extra?: string;
}

export function SinkPanel({ title, subtitle, connected, used, cap, physical, offlineMessage, extra }: Props) {
  const t = useT();
  return (
    <div className="sink-panel">
      <h3>{title} <span className="subtitle">{subtitle}</span></h3>
      {!connected ? (
        <div className="offline">{offlineMessage || "offline"}</div>
      ) : (
        <>
          <div className="meter-track">
            <div
              className="meter-seg"
              style={{ width: `${Math.max(1, pct(used, cap))}%`, background: "var(--s6)" }}
            />
          </div>
          <div className="meter-caption">
            <span>{formatBytes(used)}</span>
            <span>of {formatBytes(cap)}</span>
          </div>
          {physical !== undefined && <div className="physical-note">{t.physicalNote(formatBytes(physical))}</div>}
          {extra && <div className="physical-note">{extra}</div>}
        </>
      )}
    </div>
  );
}

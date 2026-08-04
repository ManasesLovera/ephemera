import { useAppStore } from "../store/useAppStore";
import { formatBytes } from "../lib/format";
import { MAX_RAM_BYTES } from "../types";
import { useT } from "../lib/i18n";

function Sparkline({ points, max, color }: { points: number[]; max: number; color: string }) {
  const w = 600;
  const h = 60;
  if (points.length < 2) return <svg width={w} height={h} />;
  const step = w / (points.length - 1);
  const path = points
    .map((p, i) => `${i === 0 ? "M" : "L"} ${(i * step).toFixed(1)} ${(h - (p / max) * h).toFixed(1)}`)
    .join(" ");
  return (
    <svg width="100%" height={h} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
      <path d={path} fill="none" stroke={color} strokeWidth={2} />
    </svg>
  );
}

export function Instruments() {
  const { history, ramFiles, diskFiles, dbFiles, cloudFiles } = useAppStore();
  const t = useT();
  const ramPoints = history.map((h) => h.ram);
  const rssPoints = history.map((h) => h.rss);
  const rssMax = Math.max(1, ...rssPoints, 1);

  const allRows = [
    ...ramFiles.map((f) => ({ ...f, tiers: "RAM" })),
    ...diskFiles.map((f) => ({ ...f.meta, tiers: "Disk" })),
    ...dbFiles.map((f) => ({ ...f.meta, tiers: "Database" })),
    ...cloudFiles.map((f) => ({ ...f.meta, tiers: "Cloud" })),
  ];

  return (
    <details className="drawer">
      <summary>{t.instrumentsSummary}</summary>
      <div className="drawer-content">
        <div>
          <div className="meter-caption"><span>{t.ramSeries}</span></div>
          <Sparkline points={ramPoints} max={MAX_RAM_BYTES} color="var(--s1)" />
          <div className="meter-caption"><span>{t.rssSeries(formatBytes(rssMax))}</span></div>
          <Sparkline points={rssPoints} max={rssMax} color="var(--s2)" />
          <p style={{ fontSize: 11, color: "var(--text-muted)" }}>{t.axisNote}</p>
        </div>
        <table className="file-table">
          <thead>
            <tr><th>{t.tableName}</th><th>{t.tableSize}</th><th>{t.tableTier}</th><th>{t.tableMime}</th></tr>
          </thead>
          <tbody>
            {allRows.map((r, i) => (
              <tr key={`${r.id}-${r.tiers}-${i}`}>
                <td>{r.name}</td>
                <td className="num">{formatBytes(r.size)}</td>
                <td>{r.tiers}</td>
                <td>{r.mime}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </details>
  );
}

import { formatBytes } from "../lib/format";
import { colorSlotFor } from "../lib/colors";

interface Seg {
  id: string;
  name: string;
  size: number;
}

export function Meter({ used, cap, segments }: { used: number; cap: number; segments: Seg[] }) {
  return (
    <div>
      <div className="meter-track">
        {segments.map((s) => {
          const widthPct = Math.max(0.5, (s.size / cap) * 100);
          const slot = colorSlotFor(s.id);
          return (
            <div
              key={s.id}
              className="meter-seg"
              title={`${s.name} — ${formatBytes(s.size)}`}
              style={{ width: `${widthPct}%`, background: `var(--${slot === "other" ? "other" : slot})` }}
            />
          );
        })}
      </div>
      <div className="meter-caption">
        <span>{formatBytes(used)}</span>
        <span>of {formatBytes(cap)}</span>
      </div>
    </div>
  );
}

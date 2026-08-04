import { formatBytes } from "../lib/format";
import { colorSlotFor } from "../lib/colors";
import type { FileMeta } from "../types";

interface Action {
  label: string;
  onClick: () => void;
  title?: string;
}

export function FileCard({ meta, actions }: { meta: FileMeta; actions: Action[] }) {
  const slot = colorSlotFor(meta.id);
  return (
    <div className="file-card">
      <span className="swatch" style={{ background: `var(--${slot === "other" ? "other" : slot})` }} />
      <span className="name" title={meta.name}>{meta.name}</span>
      <span className="size">{formatBytes(meta.size)}</span>
      {actions.map((a) => (
        <button key={a.label} title={a.title} onClick={a.onClick}>
          {a.label}
        </button>
      ))}
    </div>
  );
}

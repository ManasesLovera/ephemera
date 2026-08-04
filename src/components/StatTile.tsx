export function StatTile({ label, value, caption }: { label: string; value: string; caption?: string }) {
  return (
    <div className="stat-tile">
      <div className="label">{label}</div>
      <div className="value">{value}</div>
      {caption && <div className="caption">{caption}</div>}
    </div>
  );
}

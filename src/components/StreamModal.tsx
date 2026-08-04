import { formatBytes, formatMs, formatThroughput } from "../lib/format";
import type { StreamReport } from "../types";

export function StreamModal({ report, onClose }: { report: StreamReport; onClose: () => void }) {
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Streaming report — {report.file_name}</h3>
        <p style={{ fontSize: 13, color: "var(--text-secondary)" }}>
          {formatBytes(report.bytes_total)} moved in {formatMs(report.elapsed_ms)} ({formatThroughput(report.throughput_mb_s)}),
          reading/writing in fixed {formatBytes(report.chunk_size)} chunks — peak memory for this operation never
          exceeds one chunk, regardless of file size.
        </p>
        <div className="stat-pair">
          <div className="box">
            <div className="small">Streaming peak (measured)</div>
            <div className="big">{formatBytes(report.chunk_size)}</div>
          </div>
          <div className="box">
            <div className="small">Buffered would need (derived)</div>
            <div className="big">{formatBytes(report.buffered_equivalent_peak_bytes)}</div>
          </div>
          <div className="box">
            <div className="small">Files at once — streaming</div>
            <div className="big">{report.max_concurrent_streaming}</div>
          </div>
          <div className="box">
            <div className="small">Files at once — buffered</div>
            <div className="big">{report.max_concurrent_buffered}</div>
          </div>
        </div>
        <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
          RSS during transfer — baseline {formatBytes(report.rss_baseline_bytes)}, peak {formatBytes(report.rss_peak_bytes)},
          average {formatBytes(report.rss_avg_bytes)} (delta {formatBytes(Math.max(0, report.rss_peak_delta_bytes))}).
          Concurrency figures are calculated against the 10 MB RAM cap, not live-measured.
        </p>
        <div className="close-row">
          <button className="btn primary" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}

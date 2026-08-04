export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const mb = bytes / (1024 * 1024);
  if (mb < 1) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${mb.toFixed(1)} MB`;
}

export function formatMs(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(2)} s`;
}

export function formatThroughput(mbPerSec: number): string {
  if (mbPerSec < 0.01) return `${(mbPerSec * 1024).toFixed(1)} KB/s`;
  return `${mbPerSec.toFixed(2)} MB/s`;
}

export function pct(used: number, cap: number): number {
  if (cap <= 0) return 0;
  return Math.min(100, (used / cap) * 100);
}

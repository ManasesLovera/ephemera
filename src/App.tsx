import { useEffect, useState } from "react";
import "./App.css";
import { useAppStore } from "./store/useAppStore";
import { StatTile } from "./components/StatTile";
import { RamPane } from "./components/RamPane";
import { DiskPane } from "./components/DiskPane";
import { SinkPanel } from "./components/SinkPanel";
import { Instruments } from "./components/Instruments";
import { StreamModal } from "./components/StreamModal";
import { formatBytes } from "./lib/format";
import { MAX_RAM_BYTES, MAX_DISK_BYTES } from "./types";
import type { StreamReport } from "./types";

function App() {
  const { init, metrics, ramFiles, diskFiles, dbStatus, cloudStatus } = useAppStore();
  const [error, setError] = useState<string | null>(null);
  const [streamReport, setStreamReport] = useState<StreamReport | null>(null);

  useEffect(() => {
    init();
    const poll = setInterval(() => {
      useAppStore.getState().refreshDb();
      useAppStore.getState().refreshCloud();
    }, 5000);
    return () => clearInterval(poll);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const ramBytes = metrics?.ram_store_bytes ?? ramFiles.reduce((a, f) => a + f.size, 0);
  const diskBytes = metrics?.disk_store_bytes ?? diskFiles.reduce((a, f) => a + f.meta.size, 0);
  const rss = metrics?.process_rss_bytes ?? 0;

  return (
    <div className="app">
      <div className="chrome">
        <h1>Ephemera</h1>
        <span className="vault-path">RAM vs Disk vs Database vs Cloud</span>
      </div>

      {error && (
        <div className="error-banner" onClick={() => setError(null)}>
          {error} <span style={{ opacity: 0.6 }}> (click to dismiss)</span>
        </div>
      )}

      <div className="kpi-row">
        <StatTile label="RAM used" value={formatBytes(ramBytes)} caption={`of ${formatBytes(MAX_RAM_BYTES)}`} />
        <StatTile label="Disk used" value={formatBytes(diskBytes)} caption={`of ${formatBytes(MAX_DISK_BYTES)}`} />
        <StatTile label="App memory" value={formatBytes(rss)} caption="whole process tree, approximate" />
        <StatTile label="Files" value={`${ramFiles.length} / ${diskFiles.length}`} caption="ram / disk" />
      </div>

      <div className="panes">
        <RamPane onError={setError} onStreamReport={setStreamReport} />
        <DiskPane onError={setError} />
      </div>

      <div className="sinks">
        <SinkPanel
          title="Database"
          subtitle="postgres (docker)"
          connected={!!dbStatus?.connected}
          used={dbStatus?.logical_bytes ?? 0}
          cap={dbStatus?.cap ?? 100 * 1024 * 1024}
          physical={dbStatus?.physical_bytes}
          offlineMessage={dbStatus?.message ? `docker compose up -d — ${dbStatus.message}` : "docker compose up -d"}
        />
        <SinkPanel
          title="Cloud"
          subtitle="gcs bucket"
          connected={!!cloudStatus?.connected}
          used={cloudStatus?.bytes_used ?? 0}
          cap={cloudStatus?.cap ?? 100 * 1024 * 1024}
          extra={cloudStatus?.bucket ? `bucket: ${cloudStatus.bucket}` : undefined}
          offlineMessage={cloudStatus?.message || "see docs/09-gcs-tier.md"}
        />
      </div>

      <Instruments />

      {streamReport && <StreamModal report={streamReport} onClose={() => setStreamReport(null)} />}
    </div>
  );
}

export default App;

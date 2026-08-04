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
import { useT, useI18nStore } from "./lib/i18n";

function App() {
  const { init, metrics, ramFiles, diskFiles, dbStatus, cloudStatus } = useAppStore();
  const [error, setError] = useState<string | null>(null);
  const [streamReport, setStreamReport] = useState<StreamReport | null>(null);
  const t = useT();
  const { lang, setLang } = useI18nStore();

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
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span className="vault-path">{t.appSubtitle}</span>
          <button className="btn" onClick={() => setLang(lang === "en" ? "es" : "en")}>
            {lang === "en" ? "ES" : "EN"}
          </button>
        </div>
      </div>

      {error && (
        <div className="error-banner" onClick={() => setError(null)}>
          {error} <span style={{ opacity: 0.6 }}> {t.dismiss}</span>
        </div>
      )}

      <div className="kpi-row">
        <StatTile label={t.ramUsed} value={formatBytes(ramBytes)} caption={`${formatBytes(MAX_RAM_BYTES)}`} />
        <StatTile label={t.diskUsed} value={formatBytes(diskBytes)} caption={`${formatBytes(MAX_DISK_BYTES)}`} />
        <StatTile label={t.appMemory} value={formatBytes(rss)} caption={t.appMemoryCaption} />
        <StatTile label={t.files} value={`${ramFiles.length} / ${diskFiles.length}`} caption={t.filesCaption} />
      </div>

      <div className="panes">
        <RamPane onError={setError} onStreamReport={setStreamReport} />
        <DiskPane onError={setError} />
      </div>

      <div className="sinks">
        <SinkPanel
          title={t.dbTitle}
          subtitle={t.dbSubtitle}
          connected={!!dbStatus?.connected}
          used={dbStatus?.logical_bytes ?? 0}
          cap={dbStatus?.cap ?? 100 * 1024 * 1024}
          physical={dbStatus?.physical_bytes}
          offlineMessage={dbStatus?.message ? t.dockerHint(dbStatus.message) : t.dockerDefault}
        />
        <SinkPanel
          title={t.cloudTitle}
          subtitle={t.cloudSubtitle}
          connected={!!cloudStatus?.connected}
          used={cloudStatus?.bytes_used ?? 0}
          cap={cloudStatus?.cap ?? 100 * 1024 * 1024}
          extra={cloudStatus?.bucket ? `bucket: ${cloudStatus.bucket}` : undefined}
          offlineMessage={cloudStatus?.message || t.cloudDefault}
        />
      </div>

      <Instruments />

      {streamReport && <StreamModal report={streamReport} onClose={() => setStreamReport(null)} />}
    </div>
  );
}

export default App;

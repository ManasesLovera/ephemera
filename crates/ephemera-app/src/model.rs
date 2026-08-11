//! Rust-side UI-layer state model for the Slint shell.
//!
//! This module mirrors what `src/store/useAppStore.ts` holds in the Tauri build:
//! the four tier lists, the latest metrics tick, the rolling 60 s history buffer,
//! the database/cloud statuses, and the vault path. There is no IPC — every read
//! here is a direct call into `ephemera_core`, and every write is a direct
//! property set on the Slint window, marshalled onto the UI thread.
//!
//! ## The bytes rule (carried over from the Tauri app)
//!
//! The Tauri frontend held metadata only, because the webview's memory is also RAM
//! and caching file contents there would make the usage numbers lie. Removing the
//! IPC boundary does not lift that discipline: **no type in this module, and no
//! Slint property, ever holds file byte content.** Sizes, names, mime types,
//! origins, timestamps, statuses, tiers — yes. `Vec<u8>` buffers or strings
//! slurped from disk — never. The authoritative byte stores stay exclusively
//! inside `ephemera_core::state::AppState`.

use ephemera_core::types as core_types;
use slint::{Model, ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::{
    AppWindow, CloudFile as UiCloudFile, CloudStatus as UiCloudStatus, DbFile as UiDbFile,
    DbStatus as UiDbStatus, DiskFile as UiDiskFile, FileMeta as UiFileMeta,
    FileTableRow as UiFileTableRow, HistoryPoint as UiHistoryPoint, Metrics as UiMetrics,
    StreamReport as UiStreamReport,
};

/// Categorical slot colors matching App.css (--s1..--s8 and --other).
const COLOR_SLOTS: &[slint::Color] = &[
    slint::Color::from_rgb_u8(0x2a, 0x78, 0xd6), // s1
    slint::Color::from_rgb_u8(0xeb, 0x68, 0x34), // s2
    slint::Color::from_rgb_u8(0x1b, 0xaf, 0x7a), // s3
    slint::Color::from_rgb_u8(0xed, 0xa1, 0x00), // s4
    slint::Color::from_rgb_u8(0xe8, 0x7b, 0xa4), // s5
    slint::Color::from_rgb_u8(0x00, 0x83, 0x00), // s6
    slint::Color::from_rgb_u8(0x4a, 0x3a, 0xa7), // s7
    slint::Color::from_rgb_u8(0xe3, 0x49, 0x48), // s8
];
const OTHER_COLOR: slint::Color = slint::Color::from_rgb_u8(0xb7, 0xb6, 0xae);

#[derive(Default)]
pub struct ColorAssigner {
    assigned: HashMap<String, slint::Color>,
    next_slot: usize,
}

impl ColorAssigner {
    pub fn color_for(&mut self, id: &str) -> slint::Color {
        if let Some(&c) = self.assigned.get(id) {
            return c;
        }
        let c = if self.next_slot < COLOR_SLOTS.len() {
            let color = COLOR_SLOTS[self.next_slot];
            self.next_slot += 1;
            color
        } else {
            OTHER_COLOR
        };
        self.assigned.insert(id.to_string(), c);
        c
    }
}

/// 60 s of samples at 4 Hz — matches `HISTORY_LIMIT` in `useAppStore.ts`.
pub const HISTORY_LIMIT: usize = 240;

/// One point of the rolling dashboard history (metadata only).
///
/// The ring buffer is filled by `push_metrics` on every 4 Hz tick; the read side
/// (rendering the 60 s time series) belongs to the Phase 4 dashboard task, so the
/// fields are currently write-only.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct MetricPoint {
    pub ts: i64,
    pub ram: u64,
    pub rss: u64,
}

/// The UI-layer store. Holds metadata mirrors of core state and projects them into
/// Slint properties. `Send + Sync` so the 4 Hz sampler thread and the 5 s
/// db/cloud poller can push updates through [`slint::invoke_from_event_loop`].
pub struct ShellState {
    /// The single authoritative store. File bytes live only here (and in the
    /// database / cloud infrastructure) — never in this struct.
    pub core: Arc<ephemera_core::state::AppState>,
    pub vault_path: String,
    color_assigner: Mutex<ColorAssigner>,
    ram_files: Mutex<Vec<core_types::FileMeta>>,
    disk_files: Mutex<Vec<core_types::DiskFile>>,
    db_files: Mutex<Vec<core_types::DbFile>>,
    cloud_files: Mutex<Vec<core_types::CloudFile>>,
    metrics: Mutex<Option<core_types::Metrics>>,
    history: Mutex<VecDeque<MetricPoint>>,
    db_status: Mutex<Option<core_types::DbStatus>>,
    cloud_status: Mutex<Option<core_types::CloudStatus>>,
    stream_report: Mutex<Option<core_types::StreamReport>>,
}

impl ShellState {
    pub fn new(core: Arc<ephemera_core::state::AppState>, vault_path: String) -> Arc<Self> {
        Arc::new(Self {
            core,
            vault_path,
            color_assigner: Mutex::new(ColorAssigner::default()),
            ram_files: Mutex::new(Vec::new()),
            disk_files: Mutex::new(Vec::new()),
            db_files: Mutex::new(Vec::new()),
            cloud_files: Mutex::new(Vec::new()),
            metrics: Mutex::new(None),
            history: Mutex::new(VecDeque::new()),
            db_status: Mutex::new(None),
            cloud_status: Mutex::new(None),
            stream_report: Mutex::new(None),
        })
    }

    // ---- tier lists (mirrors refreshRam / refreshDisk in useAppStore.ts) ----

    /// Pull the RAM list straight from core and project it into the window.
    pub fn refresh_ram(&self, window: &AppWindow) {
        let files = ephemera_core::list_ram(&self.core);
        if let Ok(mut slot) = self.ram_files.lock() {
            *slot = files.clone();
        }
        let mut assigner = self.color_assigner.lock().unwrap();
        window.set_ram_files(meta_model(
            files
                .iter()
                .map(|f| ui_file_meta(f, &mut assigner))
                .collect(),
        ));
        self.sync_file_counts(window);
        self.sync_instrument_table(window);
    }

    /// Pull the disk list straight from core and project it into the window.
    pub fn refresh_disk(&self, window: &AppWindow) {
        let files = ephemera_core::list_disk(&self.core);
        if let Ok(mut slot) = self.disk_files.lock() {
            *slot = files.clone();
        }
        let mut assigner = self.color_assigner.lock().unwrap();
        window.set_disk_files(disk_model(
            files
                .iter()
                .map(|f| ui_disk_file(f, &mut assigner))
                .collect(),
        ));
        window.set_disk_meta_files(meta_model(
            files
                .iter()
                .map(|f| ui_file_meta(&f.meta, &mut assigner))
                .collect(),
        ));
        self.sync_file_counts(window);
        self.sync_instrument_table(window);
    }

    /// Apply a db list/status snapshot (from the poller or startup bootstrap).
    pub fn apply_db(
        &self,
        window: &AppWindow,
        status: Option<core_types::DbStatus>,
        files: Option<Vec<core_types::DbFile>>,
    ) {
        let status = status.or_else(|| {
            // No live status (postgres down / never configured): degrade to a
            // visible offline panel rather than failing, per docs/02 "optional
            // infrastructure". This mirrors the old `getDbStatus` fallback.
            Some(core_types::DbStatus {
                connected: false,
                logical_bytes: 0,
                physical_bytes: 0,
                cap: core_types::MAX_DB_BYTES,
                message: self.core.db.offline_reason(),
            })
        });
        let files = match files {
            Some(files) => {
                if let Ok(mut slot) = self.db_files.lock() {
                    *slot = files.clone();
                }
                files
            }
            None => self
                .db_files
                .lock()
                .ok()
                .map(|g| g.clone())
                .unwrap_or_default(),
        };
        if let Some(status) = &status {
            if let Ok(mut slot) = self.db_status.lock() {
                *slot = Some(status.clone());
            }
        }

        let (connected, detail) = match &status {
            Some(s) if s.connected => (
                true,
                format!("{} physical (incl. TOAST)", format_bytes(s.physical_bytes)),
            ),
            Some(s) => (
                false,
                s.message
                    .clone()
                    .unwrap_or_else(|| "database: not connected".into()),
            ),
            None => (false, "database: status unavailable".into()),
        };
        let cap = status
            .as_ref()
            .map(|s| s.cap)
            .unwrap_or(core_types::MAX_DB_BYTES);

        let mut assigner = self.color_assigner.lock().unwrap();
        window.set_db_status(status.as_ref().map(ui_db_status).unwrap_or_default());
        window.set_db_files(db_model(
            files.iter().map(|f| ui_db_file(f, &mut assigner)).collect(),
        ));
        window.set_db_meta_files(meta_model(
            files
                .iter()
                .map(|f| ui_file_meta(&f.meta, &mut assigner))
                .collect(),
        ));
        window.set_db_used_text(
            (if connected {
                format_bytes(status.as_ref().map(|s| s.logical_bytes).unwrap_or(0))
            } else {
                "offline".into()
            })
            .into(),
        );
        window.set_db_cap_text(format_bytes(cap).into());
        window.set_db_count_text(format!("{} file(s) in database", files.len()).into());
        window.set_db_detail_text(detail.into());
        self.sync_instrument_table(window);
    }

    /// Apply a cloud list/status snapshot (from the poller or startup bootstrap).
    pub fn apply_cloud(
        &self,
        window: &AppWindow,
        status: Option<core_types::CloudStatus>,
        files: Option<Vec<core_types::CloudFile>>,
    ) {
        let status = status.or_else(|| {
            Some(core_types::CloudStatus {
                connected: false,
                bytes_used: 0,
                cap: core_types::MAX_CLOUD_BYTES,
                bucket: None,
                message: self.core.cloud.offline_reason(),
            })
        });
        let files = match files {
            Some(files) => {
                if let Ok(mut slot) = self.cloud_files.lock() {
                    *slot = files.clone();
                }
                files
            }
            None => self
                .cloud_files
                .lock()
                .ok()
                .map(|g| g.clone())
                .unwrap_or_default(),
        };
        if let Some(status) = &status {
            if let Ok(mut slot) = self.cloud_status.lock() {
                *slot = Some(status.clone());
            }
        }

        let (connected, detail) = match &status {
            Some(s) if s.connected => (
                true,
                s.bucket
                    .clone()
                    .map(|b| format!("bucket: {b}"))
                    .unwrap_or_default(),
            ),
            Some(s) => (
                false,
                s.message
                    .clone()
                    .unwrap_or_else(|| "cloud: not connected".into()),
            ),
            None => (false, "cloud: status unavailable".into()),
        };
        let cap = status
            .as_ref()
            .map(|s| s.cap)
            .unwrap_or(core_types::MAX_CLOUD_BYTES);

        let mut assigner = self.color_assigner.lock().unwrap();
        window.set_cloud_status(status.as_ref().map(ui_cloud_status).unwrap_or_default());
        window.set_cloud_files(cloud_model(
            files
                .iter()
                .map(|f| ui_cloud_file(f, &mut assigner))
                .collect(),
        ));
        window.set_cloud_meta_files(meta_model(
            files
                .iter()
                .map(|f| ui_file_meta(&f.meta, &mut assigner))
                .collect(),
        ));
        window.set_cloud_used_text(
            (if connected {
                format_bytes(status.as_ref().map(|s| s.bytes_used).unwrap_or(0))
            } else {
                "offline".into()
            })
            .into(),
        );
        window.set_cloud_cap_text(format_bytes(cap).into());
        window.set_cloud_count_text(format!("{} file(s) in cloud", files.len()).into());
        window.set_cloud_detail_text(detail.into());
        self.sync_instrument_table(window);
    }

    // ---- metrics (mirrors the metrics://tick listener in useAppStore.ts) ----

    /// Called on the UI thread for each 4 Hz tick. Pushes the metrics struct,
    /// appends to the rolling history, and updates the KPI display strings.
    ///
    /// The history ring is projected into a `[HistoryPoint]` Slint model every
    /// tick so the two Instruments sparklines render the same rolling window
    /// (240 points). `rss_max` is the auto-scaled ceiling for the RSS chart —
    /// computed here in Rust because Slint has no max-over-model builtin.
    ///
    /// The Slint-side model is a single `VecModel` created once and mutated in
    /// place (push the new point, drop the oldest past `HISTORY_LIMIT`) rather
    /// than rebuilt from scratch every tick. `push_metrics` runs at 4 Hz on the
    /// UI thread (always via `invoke_from_event_loop`); reallocating a fresh
    /// `Vec` + `Rc<VecModel>` 4 times a second would itself churn the heap this
    /// app exists to measure honestly, inflating the RSS chart it's plotting.
    /// `VecModel` is `Rc`-backed (not `Send`), so it lives in a thread-local
    /// rather than a `ShellState` field — `ShellState` itself must stay
    /// `Send + Sync` to cross into the sampler/poller threads, and this model
    /// is only ever touched from the UI thread by construction.
    pub fn push_metrics(&self, window: &AppWindow, m: &core_types::Metrics) {
        if let Ok(mut slot) = self.metrics.lock() {
            *slot = Some(m.clone());
        }

        let (new_point, rss_max, len) = {
            if let Ok(mut history) = self.history.lock() {
                let point = MetricPoint {
                    ts: m.ts,
                    ram: m.ram_store_bytes,
                    rss: m.process_rss_bytes,
                };
                if history.len() == HISTORY_LIMIT {
                    history.pop_front();
                }
                history.push_back(point);
                let rss_max = history.iter().map(|p| p.rss).max().unwrap_or(1).max(1);
                (
                    UiHistoryPoint {
                        ram: point.ram as i32,
                        rss: point.rss as i32,
                    },
                    rss_max,
                    history.len(),
                )
            } else {
                (UiHistoryPoint { ram: 0, rss: 0 }, 1, 0)
            }
        };

        HISTORY_MODEL.with(|cell| {
            let mut slot = cell.borrow_mut();
            let model = slot.get_or_insert_with(|| {
                let model = Rc::new(VecModel::default());
                window.set_history(ModelRc::from(model.clone()));
                model
            });
            if model.row_count() >= HISTORY_LIMIT {
                model.remove(0);
            }
            model.push(new_point);
        });

        window.set_history_len(len as i32);
        window.set_rss_max(rss_max as i32);
        window.set_rss_series_label(
            format!(
                "Process RSS (whole tree, auto-scaled to {})",
                format_bytes(rss_max)
            )
            .into(),
        );

        window.set_metrics(ui_metrics(m));
        window.set_ram_used_text(format_bytes(m.ram_store_bytes).into());
        window.set_ram_cap_text(format_bytes(m.ram_cap).into());
        window.set_disk_used_text(format_bytes(m.disk_store_bytes).into());
        window.set_disk_cap_text(format_bytes(m.disk_cap).into());
        window.set_app_memory_text(format_bytes(m.process_rss_bytes).into());
        window.set_app_memory_caption(
            format!("{} process(es), approximated", m.process_count).into(),
        );
        window.set_db_used_text(format_bytes(m.db_store_bytes).into());
        window.set_db_cap_text(format_bytes(m.db_cap).into());
        window.set_cloud_used_text(format_bytes(m.cloud_store_bytes).into());
        window.set_cloud_cap_text(format_bytes(m.cloud_cap).into());
    }

    /// Recompute the KPI "FILES" tile and the per-pane file counts.
    fn sync_file_counts(&self, window: &AppWindow) {
        let ram = self.ram_files.lock().map(|g| g.len()).unwrap_or(0);
        let disk = self.disk_files.lock().map(|g| g.len()).unwrap_or(0);
        window.set_files_text(format!("{ram} / {disk}").into());
        window.set_ram_count_text(format!("{ram} file(s) in RAM").into());
        window.set_disk_count_text(format!("{disk} file(s) on disk").into());
    }

    /// Rebuild the cross-tier file table in the Instruments drawer from the four
    /// metadata lists. Metadata only — sizes, never bytes.
    fn sync_instrument_table(&self, window: &AppWindow) {
        let ram = self.ram_files.lock().map(|g| g.clone()).unwrap_or_default();
        let disk = self
            .disk_files
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let db = self.db_files.lock().map(|g| g.clone()).unwrap_or_default();
        let cloud = self
            .cloud_files
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        window.set_file_table_rows(rows_model(table_rows(&ram, &disk, &db, &cloud)));
    }

    /// Placeholder "refresh-all" action handler: direct core calls.
    pub fn refresh_all(&self, window: &AppWindow) {
        self.refresh_ram(window);
        self.refresh_disk(window);
        // Database/cloud refresh happens on the 5 s poller; the statuses already
        // projected here stay fresh enough for the shell.
    }

    /// Push the vault path into the window (mirrors `init` reading getConfig).
    pub fn push_vault_path(&self, window: &AppWindow) {
        window.set_vault_path(self.vault_path.clone().into());
    }

    /// Store a stream report and project it into the window modal.
    pub fn set_stream_report(&self, window: &AppWindow, report: core_types::StreamReport) {
        let ui_report = ui_stream_report(&report);
        if let Ok(mut slot) = self.stream_report.lock() {
            *slot = Some(report);
        }
        window.set_stream_report(ui_report);
        window.set_show_stream_report(true);
    }
}

// ---- projection: core types → Slint structs (metadata only) ----------------

pub fn describe_error(err: &ephemera_core::error::AppError) -> String {
    match err {
        ephemera_core::error::AppError::QuotaExceeded { needed, free, cap } => {
            format!(
                "Quota exceeded — need {} KB more, {} KB free of {} MB.",
                needed.div_ceil(1024),
                free.div_ceil(1024),
                cap / 1024 / 1024
            )
        }
        ephemera_core::error::AppError::FileTooLarge { size, cap } => {
            format!(
                "File too large: {} MB, cap is {} MB. Try \"Stream to disk\" instead.",
                size.div_ceil(1024 * 1024),
                cap / 1024 / 1024
            )
        }
        _ => err.to_string(),
    }
}

fn ui_file_meta(m: &core_types::FileMeta, assigner: &mut ColorAssigner) -> UiFileMeta {
    UiFileMeta {
        id: m.id.clone().into(),
        name: m.name.clone().into(),
        size: m.size as i32,
        mime: m.mime.clone().into(),
        created_at: format_ts(m.created_at).into(),
        origin: origin_str(&m.origin).into(),
        color: assigner.color_for(&m.id),
        size_formatted: format_bytes(m.size).into(),
    }
}

fn ui_disk_file(f: &core_types::DiskFile, assigner: &mut ColorAssigner) -> UiDiskFile {
    UiDiskFile {
        meta: ui_file_meta(&f.meta, assigner),
        persisted_at: format_ts(f.persisted_at).into(),
    }
}

fn ui_db_file(f: &core_types::DbFile, assigner: &mut ColorAssigner) -> UiDbFile {
    UiDbFile {
        meta: ui_file_meta(&f.meta, assigner),
        saved_at: format_ts(f.saved_at).into(),
    }
}

fn ui_cloud_file(f: &core_types::CloudFile, assigner: &mut ColorAssigner) -> UiCloudFile {
    UiCloudFile {
        meta: ui_file_meta(&f.meta, assigner),
        saved_at: format_ts(f.saved_at).into(),
        object_name: f.object_name.clone().into(),
    }
}

fn ui_metrics(m: &core_types::Metrics) -> UiMetrics {
    UiMetrics {
        ts: format_ts(m.ts).into(),
        ram_store_bytes: m.ram_store_bytes as i32,
        ram_cap: m.ram_cap as i32,
        disk_store_bytes: m.disk_store_bytes as i32,
        disk_cap: m.disk_cap as i32,
        db_store_bytes: m.db_store_bytes as i32,
        db_cap: m.db_cap as i32,
        db_physical_bytes: m.db_physical_bytes as i32,
        cloud_store_bytes: m.cloud_store_bytes as i32,
        cloud_cap: m.cloud_cap as i32,
        process_rss_bytes: m.process_rss_bytes as i32,
        process_count: m.process_count as i32,
    }
}

fn ui_db_status(s: &core_types::DbStatus) -> UiDbStatus {
    UiDbStatus {
        connected: s.connected,
        logical_bytes: s.logical_bytes as i32,
        physical_bytes: s.physical_bytes as i32,
        cap: s.cap as i32,
        message: s.message.clone().unwrap_or_default().into(),
    }
}

fn ui_cloud_status(s: &core_types::CloudStatus) -> UiCloudStatus {
    UiCloudStatus {
        connected: s.connected,
        bytes_used: s.bytes_used as i32,
        cap: s.cap as i32,
        bucket: s.bucket.clone().unwrap_or_default().into(),
        message: s.message.clone().unwrap_or_default().into(),
    }
}

pub fn ui_stream_report(r: &core_types::StreamReport) -> UiStreamReport {
    UiStreamReport {
        file_name: r.file_name.clone().into(),
        bytes_total: r.bytes_total as i32,
        chunk_size: r.chunk_size as i32,
        elapsed_ms: r.elapsed_ms as i32,
        throughput_mb_s: format_throughput(r.throughput_mb_s).into(),
        bytes_total_formatted: format_bytes(r.bytes_total).into(),
        chunk_size_formatted: format_bytes(r.chunk_size).into(),
        elapsed_ms_formatted: format_ms(r.elapsed_ms).into(),
        buffered_equivalent_peak_bytes: r.buffered_equivalent_peak_bytes as i32,
        buffered_equivalent_peak_bytes_formatted: format_bytes(r.buffered_equivalent_peak_bytes)
            .into(),
        max_concurrent_streaming: r.max_concurrent_streaming as i32,
        max_concurrent_buffered: r.max_concurrent_buffered as i32,
        rss_baseline_bytes: r.rss_baseline_bytes as i32,
        rss_baseline_formatted: format_bytes(r.rss_baseline_bytes).into(),
        rss_peak_bytes: r.rss_peak_bytes as i32,
        rss_peak_formatted: format_bytes(r.rss_peak_bytes).into(),
        rss_avg_bytes: r.rss_avg_bytes as i32,
        rss_avg_formatted: format_bytes(r.rss_avg_bytes).into(),
        rss_peak_delta_bytes: r.rss_peak_delta_bytes as i32,
        rss_peak_delta_formatted: format_bytes(r.rss_peak_delta_bytes.max(0) as u64).into(),
    }
}

// ---- formatting (mirrors src/lib/format.ts) --------------------------------

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    format!("{:.1} MB", kb / 1024.0)
}

pub fn format_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.2} s", ms as f64 / 1000.0)
    }
}

pub fn format_throughput(mb_per_sec: f64) -> String {
    if mb_per_sec < 0.01 {
        format!("{:.1} KB/s", mb_per_sec * 1024.0)
    } else {
        format!("{:.2} MB/s", mb_per_sec)
    }
}

/// Renders an epoch-millis timestamp as a local wall-clock string. The raw `i64`
/// stays in the Rust model (Slint's `int` is i32 and cannot carry it); the UI
/// only ever needs the rendered form.
fn format_ts(millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}

fn origin_str(o: &core_types::Origin) -> String {
    match o {
        core_types::Origin::Upload => "upload".into(),
        core_types::Origin::Stream => "stream".into(),
        core_types::Origin::Ram => "ram".into(),
        core_types::Origin::Disk => "disk".into(),
    }
}

// ---- model builders ---------------------------------------------------------

fn meta_model(items: Vec<UiFileMeta>) -> ModelRc<UiFileMeta> {
    ModelRc::from(std::rc::Rc::new(VecModel::from(items)))
}

fn disk_model(items: Vec<UiDiskFile>) -> ModelRc<UiDiskFile> {
    ModelRc::from(std::rc::Rc::new(VecModel::from(items)))
}

fn db_model(items: Vec<UiDbFile>) -> ModelRc<UiDbFile> {
    ModelRc::from(std::rc::Rc::new(VecModel::from(items)))
}

fn cloud_model(items: Vec<UiCloudFile>) -> ModelRc<UiCloudFile> {
    ModelRc::from(std::rc::Rc::new(VecModel::from(items)))
}

thread_local! {
    /// See the doc comment on `push_metrics` for why this lives here instead
    /// of on `ShellState`: `VecModel` is `Rc`-backed and only ever touched
    /// from the UI thread, which this thread-local pins it to.
    static HISTORY_MODEL: RefCell<Option<Rc<VecModel<UiHistoryPoint>>>> = const { RefCell::new(None) };
}

fn rows_model(items: Vec<UiFileTableRow>) -> ModelRc<UiFileTableRow> {
    ModelRc::from(std::rc::Rc::new(VecModel::from(items)))
}

/// Combine the four tier lists into the Instruments file table rows, in tier
/// order RAM → Disk → Database → Cloud (matching `Instruments.tsx`).
fn table_rows(
    ram: &[core_types::FileMeta],
    disk: &[core_types::DiskFile],
    db: &[core_types::DbFile],
    cloud: &[core_types::CloudFile],
) -> Vec<UiFileTableRow> {
    let mut rows = Vec::new();
    for f in ram {
        rows.push(UiFileTableRow {
            name: f.name.clone().into(),
            size_formatted: format_bytes(f.size).into(),
            tier: "RAM".into(),
            mime: f.mime.clone().into(),
        });
    }
    for f in disk {
        rows.push(UiFileTableRow {
            name: f.meta.name.clone().into(),
            size_formatted: format_bytes(f.meta.size).into(),
            tier: "Disk".into(),
            mime: f.meta.mime.clone().into(),
        });
    }
    for f in db {
        rows.push(UiFileTableRow {
            name: f.meta.name.clone().into(),
            size_formatted: format_bytes(f.meta.size).into(),
            tier: "Database".into(),
            mime: f.meta.mime.clone().into(),
        });
    }
    for f in cloud {
        rows.push(UiFileTableRow {
            name: f.meta.name.clone().into(),
            size_formatted: format_bytes(f.meta.size).into(),
            tier: "Cloud".into(),
            mime: f.meta.mime.clone().into(),
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use ephemera_core::types::{
        CloudFile as CoreCloudFile, DbFile as CoreDbFile, DiskFile as CoreDiskFile, FileMeta,
        Origin,
    };

    fn meta(id: &str) -> FileMeta {
        FileMeta {
            id: id.into(),
            name: "photo.jpg".into(),
            size: 2_200_000,
            mime: "image/jpeg".into(),
            created_at: 1_700_000_000_000,
            origin: Origin::Upload,
        }
    }

    #[test]
    fn format_bytes_matches_old_frontend() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(900), "900 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(150 * 1024), "150.0 KB");
        assert_eq!(format_bytes(4_405_000), "4.2 MB");
    }

    #[test]
    fn origin_strings_match_types_ts() {
        assert_eq!(origin_str(&Origin::Upload), "upload");
        assert_eq!(origin_str(&Origin::Stream), "stream");
        assert_eq!(origin_str(&Origin::Ram), "ram");
        assert_eq!(origin_str(&Origin::Disk), "disk");
    }

    #[test]
    fn file_meta_projection_carries_every_field_and_no_bytes() {
        let mut assigner = ColorAssigner::default();
        let ui = ui_file_meta(&meta("abc"), &mut assigner);
        assert_eq!(ui.id, "abc");
        assert_eq!(ui.name, "photo.jpg");
        assert_eq!(ui.size, 2_200_000);
        assert_eq!(ui.mime, "image/jpeg");
        assert_eq!(ui.origin, "upload");
        assert_eq!(ui.size_formatted, "2.1 MB");
        // created_at is rendered, not dropped.
        assert!(!ui.created_at.is_empty());
    }

    #[test]
    fn color_assigner_is_stable_per_id_and_caps_at_other() {
        let mut assigner = ColorAssigner::default();
        let first = assigner.color_for("a");
        let second = assigner.color_for("b");
        // Same id keeps its slot; different ids get different slots.
        assert_eq!(assigner.color_for("a"), first);
        assert_ne!(assigner.color_for("b"), first);
        let _ = second;
        // Past the 8 categorical slots, everything folds into the "other" gray.
        for i in 0..20 {
            let _ = assigner.color_for(&format!("x{i}"));
        }
        assert_eq!(assigner.color_for("y"), OTHER_COLOR);
    }

    #[test]
    fn timestamps_render_instead_of_overflowing() {
        // i64 millis ~ 1.7e12 does not fit Slint's i32; the projection must carry
        // the rendered string rather than trying to cast. Rendered as a
        // wall-clock "YYYY-MM-DD HH:MM" string (16 chars) regardless of TZ.
        let s = format_ts(1_700_000_000_000);
        assert_eq!(s.len(), 16);
        assert!(s.contains('-'));
        assert!(s.contains(':'));
    }

    #[test]
    fn table_rows_combine_all_tiers_in_order() {
        let ram = vec![meta("r1")];
        let disk = vec![CoreDiskFile {
            meta: meta("d1"),
            persisted_at: 1_700_000_000_000,
        }];
        let db = vec![CoreDbFile {
            meta: meta("b1"),
            saved_at: 1_700_000_000_000,
        }];
        let cloud = vec![CoreCloudFile {
            meta: meta("c1"),
            saved_at: 1_700_000_000_000,
            object_name: "o".into(),
        }];
        let rows = table_rows(&ram, &disk, &db, &cloud);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].tier, "RAM");
        assert_eq!(rows[1].tier, "Disk");
        assert_eq!(rows[2].tier, "Database");
        assert_eq!(rows[3].tier, "Cloud");
        assert_eq!(rows[0].name, "photo.jpg");
        assert_eq!(rows[0].size_formatted, "2.1 MB");
        assert_eq!(rows[0].mime, "image/jpeg");
    }

    #[test]
    fn format_ms_and_throughput_matches_frontend() {
        assert_eq!(format_ms(250), "250 ms");
        assert_eq!(format_ms(1500), "1.50 s");
        assert_eq!(format_throughput(0.005), "5.1 KB/s");
        assert_eq!(format_throughput(12.34), "12.34 MB/s");
    }

    #[test]
    fn stream_report_projection_carries_all_fields() {
        let report = ephemera_core::types::StreamReport {
            file_name: "test.dat".into(),
            bytes_total: 5_000_000,
            chunk_size: 262_144,
            elapsed_ms: 120,
            throughput_mb_s: 41.67,
            rss_baseline_bytes: 10_000_000,
            rss_peak_bytes: 10_262_144,
            rss_avg_bytes: 10_100_000,
            rss_peak_delta_bytes: 262_144,
            buffered_equivalent_peak_bytes: 5_000_000,
            max_concurrent_streaming: 40,
            max_concurrent_buffered: 2,
        };
        let ui = ui_stream_report(&report);
        assert_eq!(ui.file_name, "test.dat");
        assert_eq!(ui.bytes_total_formatted, "4.8 MB");
        assert_eq!(ui.chunk_size_formatted, "256.0 KB");
        assert_eq!(ui.elapsed_ms_formatted, "120 ms");
        assert_eq!(ui.throughput_mb_s, "41.67 MB/s");
        assert_eq!(ui.max_concurrent_streaming, 40);
        assert_eq!(ui.max_concurrent_buffered, 2);
    }
}

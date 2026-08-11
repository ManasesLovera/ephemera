# 02 — Architecture

> [!note]
> This document originally described a Tauri 2 + React design. The app has since
> migrated to a native [Slint](https://slint.dev) UI over the same `ephemera-core`
> logic — see [`migration/PLAN.md`](../migration/PLAN.md) for how, and
> [`migration/outputs/P6-T1/REPORT.md`](../migration/outputs/P6-T1/REPORT.md) for the
> measured memory win. Code below reflects the actual `crates/ephemera-core` and
> `crates/ephemera-app` implementation, not a pre-implementation sketch.

## Why the RAM claim is enforceable

The whole app depends on a claim: *"these bytes are in RAM and only in RAM."* That
claim has to be enforceable. In a browser app it is not — the runtime would be free to
cache, spill to IndexedDB, or swap behind our back with no way to observe it.

The RAM store lives in a real Rust `Vec<u8>`/`Arc<[u8]>` we allocated, whose lifetime we
control, whose size we can measure exactly, and which is provably destroyed when the
process exits. Originally this was split across a Tauri host process / webview
boundary; since the Slint migration the UI and the core run in **one process, one
address space** — there is no IPC anymore, so the enforcement now lives entirely in
code discipline (see the metadata-only invariant below) rather than a process boundary
doing it for us.

## Process and ownership model

```text
┌───────────────────────────────────────────────────────────────┐
│ ephemera-app process (Rust, single process — UI + core)       │
│                                                                │
│  AppState (Arc, shared with the Slint event loop)              │
│   ├── RamStore    Mutex<IndexMap<FileId, RamFile>>             │
│   │                └── bytes: Arc<[u8]>   ← THE RAM STORE      │
│   ├── DiskIndex   Mutex<IndexMap<FileId, DiskFile>>             │
│   │                └── metadata only; bytes live in vault       │
│   ├── DbStore     sqlx::PgPool                → docker postgres │
│   │                └── metadata cached; bytes live in the DB    │
│   ├── CloudStore  reqwest + JWT               → GCS bucket      │
│   │                └── metadata cached; bytes live in the bucket│
│   ├── Config      Mutex<Config> { vault_path, throttle, … }    │
│   └── Metrics     sampler thread, 4 Hz                         │
│                                                                │
│      ▲ direct fn calls        │ slint::invoke_from_event_loop  │
│      │                        ▼                                │
│  ┌────────────────────────────────────────────────┐            │
│  │ Slint window (crates/ephemera-app/ui/app.slint) │            │
│  │  ShellState (model.rs) — holds NO file bytes,   │            │
│  │  only metadata, projected into Slint properties │            │
│  └────────────────────────────────────────────────┘            │
│                                                                │
└───────────────────────────────────────────────────────────────┘
      │ writes only inside ↓          │ TCP :5432        │ HTTPS
~/.local/share/com.ephemera.app/vault/  postgres container   storage.googleapis.com
   ← THE DISK STORE               ← THE DB STORE        ← THE CLOUD STORE
```

`DbStore` and `CloudStore` are both optional infrastructure: the app's core lesson (RAM
vs. disk) must work with Postgres stopped and no GCS credentials configured. Both
degrade to an explicit "offline" panel state rather than failing app startup — see
[`08-database-tier.md`](08-database-tier.md) and [`09-gcs-tier.md`](09-gcs-tier.md).

**Invariant:** the UI layer never holds the authoritative copy of a file's bytes. It
holds `FileMeta` records only, projected into Slint's `FileMeta`/`DiskFile`/`DbFile`/
`CloudFile` structs (`crates/ephemera-app/ui/app.slint`). This keeps the accounting
honest — if the UI cached file contents, "RAM usage" would be a lie, even though there
is no serialization boundary anymore forcing the separation.

## Core types

```rust
pub type FileId = String; // uuid v4 as string

#[derive(Clone, Serialize)]
pub struct FileMeta {
    pub id: FileId,
    pub name: String,        // sanitised, no path separators
    pub size: u64,
    pub mime: String,        // sniffed, best-effort
    pub created_at: i64,     // unix millis
    pub origin: Origin,      // Upload | RestoredFromDisk
}

pub struct RamFile {
    pub meta: FileMeta,
    /// The store. `Arc<[u8]>` so a persist-to-disk can take a cheap handle
    /// and release the store lock before doing slow I/O — no second copy.
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Serialize)]
pub struct DiskFile {
    pub meta: FileMeta,
    pub path: PathBuf,       // always inside vault_path
    pub persisted_at: i64,
}
```

`IndexMap` (from the `indexmap` crate) rather than `HashMap` so the UI list has a
stable, insertion-ordered sequence without the UI layer having to sort.

### Why `Arc<[u8]>` matters

Persisting a 9 MB file must not transiently allocate another 9 MB — that would blow past
the 10 MB cap in real memory while the accounting still said 9 MB, making our own
dashboard wrong. Taking an `Arc` clone is a pointer copy. The lock is then released
before the write begins, so the UI stays responsive and other operations are not
blocked.

This is also worth surfacing in the UI as a teaching point: *a reference is not a copy.*

## Quota enforcement

Both stores enforce the same way — check before allocating, never after:

```rust
fn assert_fits(current: u64, incoming: u64, cap: u64) -> Result<(), AppError> {
    if incoming > cap {
        return Err(AppError::FileTooLarge { size: incoming, cap });
    }
    if current + incoming > cap {
        return Err(AppError::QuotaExceeded {
            needed: (current + incoming) - cap,
            free: cap - current,
            cap,
        });
    }
    Ok(())
}
```

The error carries the numbers so the UI can say "3.2 MB too large — free up space or
delete a file" instead of "upload failed". A quota error is the app's most important
teaching moment; it deserves a real message and a good-looking UI state, not a toast.

For a multi-file drop, validate the whole batch against free space first, then accept
files in order until one does not fit, and report the rejected ones as a group.

## Getting file bytes into the store

Since UI and core run in the same process, this is now the simplest part of the app:
`crates/ephemera-app/src/main.rs`'s `on_upload_ram_files` handler opens an `rfd` file
picker, gets real filesystem paths back, and calls
`ephemera_core::upload_to_ram(&state, path)` directly — Rust reads the file itself.
No serialization, no base64, no IPC copy at all. This was already the direction the
original Tauri-era design was heading (see below) and the Slint migration made it the
only path.

> [!note]
> **Historical Tauri detail, kept for context.** The original Tauri build used its
> native drag-drop event (filesystem paths, not contents) for the same reason: reading
> the file directly in Rust avoids a serialization copy across the webview/host
> boundary. Tauri intercepted OS drag-and-drop at the webview level, suppressing HTML5
> `dragover`/`drop` events — a real gotcha for that build, now moot since there is no
> webview.
>
> **Slint limitation (current):** Slint 1.17's `DataTransfer`/`DropArea` API carries
> only images, plain text, and same-process payloads — no file paths — so real OS-level
> drag-and-drop of files from outside the window cannot be implemented on this Slint
> version. The click-to-browse file picker (`rfd`) is the working equivalent; in-app
> dragging a file between panes is a separate, unaffected mechanism (Slint's
> `DragArea`/`DropArea` with an in-process payload).

## Chunking, progress, and honest latency

Both directions copy in chunks (256 KB, see `crates/ephemera-core/src/stream.rs`)
rather than in one call, for three reasons: the progress bar becomes real, the live
memory chart has something to animate, and an optional throttle can be applied between
chunks to make disk latency perceptible.

```rust
const CHUNK: usize = 256 * 1024;

for (i, chunk) in bytes.chunks(CHUNK).enumerate() {
    writer.write_all(chunk)?;
    if let Some(delay) = throttle_per_chunk { tokio::time::sleep(delay).await; }
    on_progress(TransferProgress {
        id: id.clone(),
        direction: Direction::RamToDisk,
        bytes_done: ((i + 1) * CHUNK).min(total) as u64,
        bytes_total: total as u64,
    });
}
writer.flush()?;
file.sync_all()?; // real fsync — this is where disk actually costs you
```

`sync_all()` is not optional. Without it the write lands in the OS page cache and
returns almost as fast as a memory copy, which would make the app teach the *opposite*
of the truth. With it, the timing reflects reaching durable storage.

Any artificial throttle **must be off by default and labelled in the UI** when on. See
[`06-open-questions.md`](06-open-questions.md).

## Core API surface

`crates/ephemera-core` exposes plain async/sync functions over `&AppState` (no
`#[tauri::command]`, no `tauri::State`/`Window` coupling — extracted in the migration's
Phase 1 specifically so the logic has no Tauri dependency). `crates/ephemera-app`'s
`main.rs` wires each one to a Slint callback:

| Function | Args | Returns | Notes |
| --- | --- | --- | --- |
| `upload_to_ram` | path | `FileMeta` | Enforces both caps |
| `list_ram` | — | `Vec<FileMeta>` | Cheap; called on refresh |
| `delete_from_ram` | `id` | `()` | Frees quota immediately |
| `flush_ram` | — | `()` | "Pull the plug"; drops the whole store |
| `persist_to_disk` | `id` | `DiskFile` | RAM → disk; fsyncs |
| `stream_upload_to_disk` | source path, progress closure | `StreamReport` | Bypasses RAM entirely; see [`07-streaming.md`](07-streaming.md) |
| `list_disk` | — | `Vec<DiskFile>` | Reads the index |
| `rescan_vault` | — | `Vec<DiskFile>` | Re-derives index from the folder |
| `delete_from_disk` | `id` | `()` | Unlinks the real file |
| `save_to_db` | `id`, source (`ram`\|`disk`) | `DbFile` | One-way; see [`08-database-tier.md`](08-database-tier.md) |
| `list_db` | — | `Vec<DbFile>` | |
| `delete_from_db` | `id` | `()` | |
| `get_db_status` | — | `DbStatus` | Drives the offline banner |
| `save_to_cloud` | `id`, source (`ram`\|`disk`) | `CloudFile` | One-way; see [`09-gcs-tier.md`](09-gcs-tier.md) |
| `list_cloud` | — | `Vec<CloudFile>` | |
| `delete_from_cloud` | `id` | `()` | |
| `get_cloud_status` | — | `CloudStatus` | Drives the offline/misconfigured banner |
| `get_config` | — | `Config` | |
| `set_vault_path` | `path` | `Config` | Validates writable; rescans. In-memory only, not persisted across restarts — a pre-existing gap from before the migration, not introduced by it (see `docs/10-implementation-status.md`) |
| `get_vault_path` | — | `String` | Used by "Open folder" |
| `get_metrics` | — | `Metrics` | One-shot; the 4 Hz sampler is the normal path |

> [!important]
> There is **no `load_to_ram` from Disk, Database, or Cloud**. The tier graph is
> one-directional — see
> [`01-requirements.md`](01-requirements.md#moving-files-and-the-tier-graph). The only
> way a file re-enters RAM is a fresh `upload_to_ram`.

Every function returns `Result<T, AppError>`; `main.rs` converts errors to a display
string via `describe_error` and sets the `error-message` Slint property.

## Marshalling updates onto the UI thread

There's no IPC event system anymore, but there is still a thread-marshalling concern:
Slint properties can only be touched from the UI thread, while the 4 Hz metrics
sampler and the 5 s db/cloud poller run on background threads (a tokio runtime and a
dedicated sampler thread respectively). Each marshals its result onto the UI thread via
[`slint::invoke_from_event_loop`](https://docs.slint.dev/latest/docs/rust/slint/fn.invoke_from_event_loop.html)
before setting any property.

| Update | Rate | Marshalled via |
| --- | --- | --- |
| Metrics tick | 4 Hz | `spawn_sampler` closure → `invoke_from_event_loop` |
| DB/Cloud status + file list | 5 s | tokio task → `invoke_from_event_loop` |
| Per-transfer progress | per chunk | (not currently surfaced as a live progress bar; `StreamReport` is shown once, on completion) |

## Metrics sampler

```rust
#[derive(Clone, Serialize)]
pub struct Metrics {
    pub ts: i64,
    pub ram_store_bytes: u64,     // sum of RamFile sizes — what we chose to hold
    pub ram_cap: u64,
    pub disk_store_bytes: u64,    // sum of vault file sizes
    pub disk_cap: u64,
    pub process_rss_bytes: u64,   // whole process tree — what we actually cost
    pub process_count: usize,
}
```

A background thread samples every 250 ms with the `sysinfo` crate and calls back into
`push_metrics`, which pushes the tick into `ShellState`'s history ring and Slint
properties (see `crates/ephemera-app/src/model.rs`).

> [!important]
> `process_rss_bytes` **must sum the whole process tree**, not just our PID. This
> mattered even more under Tauri, where WebKitGTK ran web content in separate child
> processes (`WebKitWebProcess`, `WebKitNetworkProcess`) — the migration's whole
> premise was that dropping those children cuts memory dramatically, confirmed at
> [`migration/outputs/P6-T1/REPORT.md`](../migration/outputs/P6-T1/REPORT.md) (123 MiB
> vs. 393 MiB tree RSS at idle). The Slint app is normally a single process, but the
> tree-summing logic stays in place since it's still correct and harmless.
>
> RSS across a process tree double-counts shared pages, so label it in the UI as an
> approximation. Do not present it as an exact figure.

The gap between `ram_store_bytes` and `process_rss_bytes` is still one of the app's
best teaching artefacts, just a much smaller gap post-migration: store 4 MB of files,
watch the process sit well below where the old Tauri build would have. That
difference is the runtime, the allocator, and the UI toolkit's own overhead — a real
answer to "why does a simple app use more memory than the bytes you gave it?"

## Dashboard rendering note

Slint has no built-in charting library and (as of 1.17) its `Path` element does not
allow a `for` loop inside it, so a dynamic SVG-command line chart isn't viable for the
rolling 60 s history sparklines. `Instruments` (`app.slint`) instead renders each
stacked time-series chart as a `for point in history: Rectangle { ... }` bar-style
sparkline, matching the segmented-bar approach already used for `Meter`. The history
model itself is a single persistent `VecModel` mutated in place (push/pop) rather than
rebuilt every 4 Hz tick — see the doc comment on `ShellState::push_metrics` in
`model.rs` for why that mattered (a naive rebuild would itself churn the heap this app
exists to measure honestly).

## Disk index and the vault as source of truth

The vault folder is authoritative. On startup, on `set_vault_path`, and on window focus,
`rescan_vault` walks the folder, sums sizes, and rebuilds `DiskIndex`.

A sidecar `.ephemera-index.json` inside the vault stores extra metadata (original name,
persisted-at, source file id). It must be **excluded from the disk-usage total** and
hidden from the UI file list, or the numbers will not add up. If a file exists on disk
with no sidecar entry, still show it — an externally-added file is legitimate, and
noticing it is the point of rescanning.

Filename collisions: suffix with ` (2)`, ` (3)` rather than overwriting. Silent
overwrite is data loss, which is a bad thing for an app about not losing data to do.

## Safety rules

- Sanitise every filename: strip path separators, `..`, and control characters; reject
  empty results. Then join to `vault_path` and canonicalise, and assert the result is
  still inside the canonicalised vault before any write.
- Treat every filesystem call as fallible — the vault can vanish, fill up, or become
  read-only mid-session. Surface these as normal UI errors, never a panic.

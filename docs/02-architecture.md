# 02 — Architecture

> [!note]
> Code in this document is **design sketch, not verified compiling code**. Tauri 2 API
> details (raw IPC bodies, `Channel`, header access) should be checked against the
> current Tauri docs during implementation — they are the parts most likely to have
> drifted.

## Why Tauri is the right shape for this

The whole app depends on a claim: *"these bytes are in RAM and only in RAM."* That
claim has to be enforceable. In a browser app it is not — the runtime would be free to
cache, spill to IndexedDB, or swap behind our back with no way to observe it.

Tauri splits the app across a real boundary: a Rust host process that owns memory
explicitly, and a webview that owns only presentation. Putting the RAM store in Rust
means the metaphor is backed by an actual `Vec<u8>` we allocated, whose lifetime we
control, whose size we can measure exactly, and which is provably destroyed when the
process exits. **The architectural split and the pedagogical split are the same split.**
That is the reason to use Tauri here rather than Electron or a web page.

## Process and ownership model

```text
┌───────────────────────────────────────────────────────────────┐
│ Ephemera process (Rust host)                                  │
│                                                                │
│  AppState (managed, Arc)                                      │
│   ├── RamStore    Mutex<IndexMap<FileId, RamFile>>             │
│   │                └── bytes: Arc<[u8]>   ← THE RAM STORE      │
│   ├── DiskIndex   Mutex<IndexMap<FileId, DiskFile>>             │
│   │                └── metadata only; bytes live in vault       │
│   ├── DbStore     sqlx::PgPool                → docker postgres │
│   │                └── metadata cached; bytes live in the DB    │
│   ├── CloudStore  google-cloud-storage client → GCS bucket      │
│   │                └── metadata cached; bytes live in the bucket│
│   ├── Config      Mutex<Config> { vault_path, throttle, … }    │
│   └── Metrics     sampler thread, 4 Hz                         │
│                                                                │
│      ▲ IPC commands          │ events / channels               │
│      │                       ▼                                 │
│  ┌────────────────────────────────────────────────┐            │
│  │ WebView (WebKitGTK on Linux)                    │            │
│  │  React UI — holds NO file bytes, only metadata  │            │
│  │  (exception: transient blob URLs for previews)  │            │
│  └────────────────────────────────────────────────┘            │
│                                                                │
└───────────────────────────────────────────────────────────────┘
      │ writes only inside ↓          │ TCP :5432        │ HTTPS
~/.local/share/ephemera/vault/   postgres container   storage.googleapis.com
   ← THE DISK STORE               ← THE DB STORE        ← THE CLOUD STORE
```

`DbStore` and `CloudStore` are both optional infrastructure: the app's core lesson (RAM
vs. disk) must work with Postgres stopped and no GCS credentials configured. Both
degrade to an explicit "offline" panel state rather than failing app startup — see
[`08-database-tier.md`](08-database-tier.md) and [`09-gcs-tier.md`](09-gcs-tier.md).

**Invariant:** the frontend never holds the authoritative copy of a file's bytes. It
holds `FileMeta` records only. This keeps the accounting honest — if the UI cached file
contents, "RAM usage" would be a lie, because the webview's memory is also RAM.

## Core types

```rust
pub type FileId = String; // uuid v4 as string; simplest across the IPC boundary

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
stable, insertion-ordered sequence without the frontend having to sort.

### Why `Arc<[u8]>` matters

Persisting a 9 MB file must not transiently allocate another 9 MB — that would blow past
the 10 MB cap in real memory while the accounting still said 9 MB, making our own
dashboard wrong. Taking an `Arc` clone is a pointer copy. The lock is then released
before the write begins, so the UI stays responsive and other commands are not blocked.

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

## Getting file bytes across the IPC boundary

This is the single most important implementation detail, and the easiest to get wrong.

Tauri's default IPC serialises arguments as JSON. Sending a 10 MB file that way means
base64 encoding it: **+33% size**, a large string in the webview, a large string in
Rust, and a decode step. Three copies of the file exist at peak, and our "RAM usage"
number would be badly understated relative to what the process actually consumed.

Use Tauri 2's **raw request body** instead, so a `Uint8Array` crosses as bytes:

```rust
#[tauri::command]
async fn upload_to_ram(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
    on_progress: tauri::ipc::Channel<TransferProgress>,
) -> Result<FileMeta, AppError> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(AppError::BadRequest);
    };
    let name = request.headers().get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unnamed");
    // sanitise name, assert_fits, chunk-copy into the store emitting progress
    todo!()
}
```

```ts
// frontend
const buf = await file.arrayBuffer();
await invoke<FileMeta>("upload_to_ram", new Uint8Array(buf), {
  headers: { "x-file-name": file.name },
});
```

> [!warning]
> Verify the exact raw-body and header APIs against the Tauri 2 docs at build time. If
> raw bodies turn out to be awkward, the fallback is the **Tauri drag-drop path route**
> described below, which avoids the boundary problem entirely.

### Alternative: let Rust read the file itself

Tauri's native drag-drop event delivers **filesystem paths**, not contents. Rust can
then `File::open` and read into the store directly. Advantages: no IPC copy at all,
trivially streamable, and it is genuinely the more honest design. Disadvantage: it only
works for OS drags, not for the click-to-browse picker (which needs
`tauri-plugin-dialog`'s file picker, also returning paths — so actually it covers both).

**Recommendation:** implement the path-based route as the primary mechanism. It is
simpler, faster, uses less memory, and makes the "bytes go straight into the store"
claim airtight. Keep raw-body IPC as a fallback only if a path is unavailable.

> [!important]
> **Tauri drag-drop gotcha.** Tauri intercepts OS drag-and-drop at the webview level by
> default, which *suppresses HTML5 `dragover`/`drop` events entirely*. React dropzone
> libraries will appear to be broken with no error. You must choose one:
>
> - keep `dragDropEnabled: true` and handle `tauri://drag-drop` in Rust/JS (get paths), or
> - set `app.windows[].dragDropEnabled: false` in `tauri.conf.json` to let HTML5 DnD work
>   (get `File` objects, must send bytes).
>
> Since the recommendation above is path-based, keep it **enabled** and drive the drop
> zone's hover styling from the `tauri://drag-enter` / `drag-leave` events. Note that
> in-app dragging between the two panes (dnd-kit) is unaffected either way — that is
> pointer-event based, not HTML5 DnD.

## Chunking, progress, and honest latency

Both directions copy in chunks (suggested 256 KB) rather than in one call, for three
reasons: the progress bar becomes real, the live memory chart has something to animate,
and an optional throttle can be applied between chunks to make disk latency perceptible.

```rust
const CHUNK: usize = 256 * 1024;

for (i, chunk) in bytes.chunks(CHUNK).enumerate() {
    writer.write_all(chunk)?;
    if let Some(delay) = throttle_per_chunk { tokio::time::sleep(delay).await; }
    on_progress.send(TransferProgress {
        id: id.clone(),
        direction: Direction::RamToDisk,
        bytes_done: ((i + 1) * CHUNK).min(total) as u64,
        bytes_total: total as u64,
    })?;
}
writer.flush()?;
file.sync_all()?; // real fsync — this is where disk actually costs you
```

`sync_all()` is not optional. Without it the write lands in the OS page cache and
returns almost as fast as a memory copy, which would make the app teach the *opposite*
of the truth. With it, the timing reflects reaching durable storage.

Any artificial throttle **must be off by default and labelled in the UI** when on. See
[`06-open-questions.md`](06-open-questions.md).

## IPC surface

| Command | Args | Returns | Notes |
| --- | --- | --- | --- |
| `upload_to_ram` | path or raw bytes, `Channel` | `FileMeta` | Enforces both caps; emits progress |
| `list_ram` | — | `Vec<FileMeta>` | Cheap; called on mount and after mutations |
| `delete_from_ram` | `id` | `()` | Frees quota immediately |
| `flush_ram` | — | `()` | "Pull the plug"; drops the whole store |
| `persist_to_disk` | `id`, `Channel` | `DiskFile` | RAM → disk; fsyncs; returns timing |
| `stream_upload_to_disk` | source path, `Channel` | `StreamReport` | Bypasses RAM entirely; see [`07-streaming.md`](07-streaming.md) |
| `list_disk` | — | `Vec<DiskFile>` | Reads the index |
| `rescan_vault` | — | `Vec<DiskFile>` | Re-derives index from the folder |
| `delete_from_disk` | `id` | `()` | Unlinks the real file |
| `save_to_db` | `id`, source (`ram`\|`disk`), `Channel` | `DbFile` | One-way; see [`08-database-tier.md`](08-database-tier.md) |
| `list_db` | — | `Vec<DbFile>` | |
| `delete_from_db` | `id` | `()` | |
| `get_db_status` | — | `DbStatus` | Drives the offline banner |
| `save_to_cloud` | `id`, source (`ram`\|`disk`), `Channel` | `CloudFile` | One-way; see [`09-gcs-tier.md`](09-gcs-tier.md) |
| `list_cloud` | — | `Vec<CloudFile>` | |
| `delete_from_cloud` | `id` | `()` | |
| `get_cloud_status` | — | `CloudStatus` | Drives the offline/misconfigured banner |
| `get_config` | — | `Config` | |
| `set_vault_path` | `path` | `Config` | Validates writable; rescans |
| `reveal_vault` | — | `()` | Opens the folder in the system file manager |
| `get_metrics` | — | `Metrics` | One-shot; the stream is the normal path |

> [!important]
> There is **no `load_to_ram` from Disk, Database, or Cloud**. An earlier draft of this
> spec included a disk → RAM "load" command; that is superseded by the one-directional
> tier graph in [`01-requirements.md`](01-requirements.md#moving-files-and-the-tier-graph).
> The only way a file re-enters RAM is a fresh `upload_to_ram`.

Every command returns `Result<T, AppError>` with a `serde`-serialisable error enum, so
the frontend gets structured errors it can render properly rather than strings.

## Events

| Event | Payload | Rate | Purpose |
| --- | --- | --- | --- |
| `metrics://tick` | `Metrics` | 4 Hz | Drives the live charts |
| `store://changed` | `{ store: "ram" \| "disk" }` | on mutation | Tells the UI to refetch a list |
| per-transfer `Channel` | `TransferProgress` | per chunk | Scoped to one transfer, not global |

Using a `Channel` for progress rather than a global event means two simultaneous
transfers do not have to be demultiplexed by the frontend.

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

A background thread samples every 250 ms with the `sysinfo` crate and emits
`metrics://tick`.

> [!important]
> `process_rss_bytes` **must sum the whole process tree**, not just our PID. On Linux,
> WebKitGTK runs the web content in separate child processes (`WebKitWebProcess`,
> `WebKitNetworkProcess`). Reporting only the Rust process would understate real memory
> use by a large margin and undercut the "the app itself costs memory" lesson. Walk
> `sysinfo`'s process list collecting descendants of our PID.
>
> RSS across a process tree double-counts shared pages, so label it in the UI as an
> approximation. Do not present it as an exact figure.

The gap between `ram_store_bytes` and `process_rss_bytes` is one of the app's best
teaching artefacts: store 4 MB of files, watch the process sit at 150–250 MB. That
difference is the runtime, the webview, the compositor buffers, and the allocator — and
it is a real answer to "why does a simple app use so much memory?"

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
- Scope `tauri-plugin-fs` permissions to the vault path only.
- Treat every filesystem call as fallible — the vault can vanish, fill up, or become
  read-only mid-session. Surface these as normal UI errors, never a panic.

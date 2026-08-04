# 01 — Requirements

Requirements are tagged **MUST** (core, from the original brief), **SHOULD** (strongly
recommended, adds teaching value), and **COULD** (nice to have, defer if time-pressed).

## Hard limits

| Constant | Value | Bytes | Rationale |
| --- | --- | --- | --- |
| `MAX_RAM_BYTES` | 10 MB | `10 * 1024 * 1024` | Reachable in a few files; RAM is the scarce tier |
| `MAX_DISK_BYTES` | 20 MB | `20 * 1024 * 1024` | Deliberately **2x** RAM — see note below |
| `MAX_SINGLE_FILE` | 10 MB | same as RAM cap | A file that cannot fit in RAM can never be uploaded **to RAM**. It can still reach disk via the streaming path — see [`07-streaming.md`](07-streaming.md) |
| `MAX_DB_BYTES` | 100 MB | `100 * 1024 * 1024` | Largest local cap — see [`08-database-tier.md`](08-database-tier.md) |
| `MAX_CLOUD_BYTES` | 100 MB (UI convention) | `100 * 1024 * 1024` | GCS has no real capacity ceiling at this scale; the cap exists only so the meter means something — see [`09-gcs-tier.md`](09-gcs-tier.md) |
| `STREAM_CHUNK_BYTES` | 256 KB | `256 * 1024` | Fixed buffer size for the streaming path, independent of file size |

> [!important]
> Disk being twice RAM is a teaching decision, not an arbitrary one. It means the disk
> pane can hold more than the RAM pane can hold *at once*, so filling disk to 20 MB
> requires the user to upload, persist, clear RAM, and upload again. That loop is
> literally how buffered I/O works, and the user discovers it by being forced into it.

Use binary MB (MiB) internally. Display as "10.0 MB" — do not surface the MiB/MB
distinction in the UI; it is noise against the actual lesson.

## RAM store

- **MUST** hold uploaded file bytes entirely in process memory. No temp files, no
  serialisation to disk, no OS cache warming as a side effect of our own code.
- **MUST** reject an upload that would push the store over `MAX_RAM_BYTES`, with an
  error that names the deficit ("needs 3.2 MB more than is free") rather than a generic
  failure.
- **MUST** reject a single file larger than `MAX_SINGLE_FILE` before reading its bytes,
  not after.
- **MUST** lose all contents when the app process exits. This is the feature.
- **MUST** survive a webview reload (F5 / devtools reload). State lives in Rust, not in
  JS, so refreshing the UI must not clear the RAM pane — see
  [`05-teaching-notes.md`](05-teaching-notes.md), this contrast is a deliberate demo.
- **SHOULD** support deleting an individual file from RAM (freeing quota).
- **SHOULD** offer a **"Pull the plug"** action that drops the entire store at once,
  with a deliberately abrupt animation. Simulates power loss without quitting the app.
- **COULD** warn on app close if RAM holds files that were never persisted — but the
  warning must be dismissible and must *not* prevent the loss. Losing the file is the
  lesson; a dialog that saves the user from it would destroy the point.

## Disk store

- **MUST** write to a single user-configurable folder ("the vault"), chosen via a native
  folder picker.
- **MUST** default to a sensible path on first run and create it if absent. Suggested:
  `$XDG_DATA_HOME/ephemera/vault`, i.e. `~/.local/share/ephemera/vault` on this machine.
- **MUST** enforce `MAX_DISK_BYTES` across the folder's total contents, refusing a
  persist that would exceed it.
- **MUST** persist across app restarts (trivially true — it is a real folder).
- **MUST** never write outside the configured vault folder. Path traversal via a crafted
  filename (`../../.bashrc`) has to be impossible; sanitise the stored filename.
- **SHOULD** treat the folder itself as the source of truth, rescanning on startup and
  on window focus, so that files added or deleted with an external file manager show up.
  This teaches that the filesystem is shared mutable state that other programs can touch
  — the opposite of the private RAM store.
- **SHOULD** let the user open the vault folder in the system file manager, so they can
  verify with their own eyes that the files are really there.
- **SHOULD** support deleting a file from disk.
- **COULD** show a per-file "on disk since" timestamp.

## Moving files and the tier graph

There are now four tiers — **RAM, Disk, Database, Cloud (GCS)** — and movement between
them is **one-directional toward durability**. This is a deliberate rule change from an
earlier draft of this spec, which allowed disk → RAM; that is superseded (see
[`06-open-questions.md`](06-open-questions.md), item 6, marked superseded).

```text
        ┌────────────────┐
        │      RAM        │  volatile · private · 10 MB
        └───────┬────┬────┘
                │    │
     ┌──────────┘    └──────────┐
     ▼                          ▼
┌─────────┐              ┌────────────┐        ┌──────────┐
│  Disk   │─────────────▶│  Database  │        │  Cloud   │
│ 20 MB   │──────────────┼───────────▶│  100MB │  100 MB* │
└─────────┘              └────────────┘        └──────────┘
     │                                               ▲
     └───────────────────────────────────────────────┘
```

Valid edges: **RAM → Disk, RAM → Database, RAM → Cloud, Disk → Database, Disk → Cloud.**
No edge leads back to RAM, from any tier. No edge exists between Database and Cloud in
this version.

- **MUST** provide an explicit action per edge ("persist" for RAM → Disk, "save to
  database" for → Database, "save to cloud" for → Cloud). Never automatic.
- **MUST NOT** provide any action that moves a file back into RAM. Once a file leaves
  RAM — however it leaves — the only way to see it in the RAM pane again is to upload it
  again. This is intentional: it keeps "in RAM" meaning "the app currently holds this
  in memory because you just gave it this", not a cache that silently refills.
- **SHOULD** make the primary RAM → Disk gesture a **drag from the RAM pane to the disk
  pane**, with a button as the accessible/discoverable equivalent. Dragging a file
  across the screen *is* the mental model.
- **SHOULD** expose "save to database" and "save to cloud" as buttons on every RAM and
  disk file card (drag targets for these are a stretch goal, not required — a compact
  destination panel does not need the same full drag treatment as the disk pane).
- **SHOULD** report per-transfer stats on completion: bytes, elapsed ms, MB/s. For
  Database and Cloud, also report the destination-specific numbers described in their
  own docs (logical vs. physical size for DB; network throughput for Cloud).
- **COULD** offer "persist all" for filling disk quickly during a demo.

## Upload

- **MUST** accept files via drag-and-drop from the OS onto the RAM pane.
- **MUST** also accept a click-to-browse file picker (drag-and-drop alone is not
  accessible, and does not work well when projecting to a class).
- **SHOULD** accept multiple files at once, validating the batch against remaining quota
  as a whole and reporting exactly which ones did not fit.
- **SHOULD** stream/chunk the transfer so the progress bar and the live memory chart
  reflect real progress rather than snapping 0 → 100.
- **COULD** show a thumbnail for image files (rendered from the in-RAM bytes via a blob
  URL, which is itself a nice "this pixel data is in memory" moment).

## Streaming upload (RAM-bypass path)

Full detail in [`07-streaming.md`](07-streaming.md). Summary:

- **MUST** offer a second upload action — "Stream to disk" — alongside "Upload to RAM",
  that reads the source file in fixed-size chunks and writes each directly to the vault,
  never buffering the whole file in memory.
- **MUST** allow this path to accept files larger than `MAX_SINGLE_FILE`, bounded only
  by `MAX_DISK_BYTES`.
- **MUST** show a completion report with: elapsed time, throughput, measured RSS
  baseline/peak/average during the transfer, the buffered-equivalent peak (== file
  size, clearly labelled as derived not measured), and the concurrency comparison —
  how many same-size files could run at once via streaming vs. via the RAM-buffered
  path, both computed against `MAX_RAM_BYTES`.

## Database tier (Postgres via Docker)

Full detail in [`08-database-tier.md`](08-database-tier.md). Summary:

- **MUST** run Postgres via `docker compose`, storing file bytes as `BYTEA`.
- **MUST** enforce `MAX_DB_BYTES` (100 MB) against the logical sum of stored file sizes.
- **MUST** be reachable from both RAM and disk file cards via a "save to database"
  button; **MUST NOT** be reachable back to RAM.
- **SHOULD** show both the logical byte total (what counts against the cap) and the
  physical on-disk size (`pg_total_relation_size`, includes TOAST/page overhead) as two
  distinct, separately labelled numbers.
- **MUST** degrade gracefully to an "offline" UI state if the container is not running,
  without affecting the RAM/disk core of the app.

## Cloud tier (Google Cloud Storage)

Full detail in [`09-gcs-tier.md`](09-gcs-tier.md). Summary:

- **MUST** upload to a single configured GCS bucket via a scoped service-account key,
  never the developer's personal credentials.
- **MUST** be reachable from both RAM and disk file cards via a "save to cloud" button;
  **MUST NOT** be reachable back to RAM.
- **SHOULD** show a self-imposed demo cap (`MAX_CLOUD_BYTES`, default 100 MB) with a UI
  note that this is a convention, not a real GCS limit.
- **MUST** degrade gracefully if the key file is missing or the network is unreachable.
- **MUST NOT** commit the service-account key file to version control.

## Dashboard and telemetry

- **MUST** show RAM usage against its 10 MB cap, broken down **per file**.
- **MUST** show disk usage against its 20 MB cap, broken down **per file**.
- **MUST** show database usage (logical + physical) against its 100 MB cap, and cloud
  usage against its configured cap, both broken down **per file**.
- **MUST** update in real time *during* an upload and *during* every transfer between
  tiers, not only on completion.
- **MUST** show the total memory consumption of the app process itself, distinct from
  the sum of stored file bytes.
- **SHOULD** plot a rolling time-series of RAM store bytes and process RSS on a shared
  time axis, so the spike caused by an upload is visible against the baseline.
- **SHOULD** show a throughput comparison across all four tiers' write/read operations,
  plus the streaming-vs-buffered comparison from the streaming report.
- See [`03-ui-and-visualization.md`](03-ui-and-visualization.md) for the full chart
  inventory and how each one is drawn.

## Configuration

- **MUST** persist the vault path between runs.
- **SHOULD** keep app config in the OS config dir, *not* in the vault — mixing app state
  into the vault would corrupt the disk-usage accounting and muddy the metaphor.
- **COULD** allow the two caps to be edited, for a teacher who wants a different demo.
  If added, treat 10/20 MB as defaults and make "reset to defaults" prominent.

## Non-functional

- **MUST** stay responsive during transfers — no blocking the UI thread on a 10 MB
  write. Long work runs off the main thread.
- **MUST** handle the vault folder being deleted or made read-only underneath the app
  without crashing.
- **SHOULD** be legible when projected: large type, high contrast, and no critical
  information conveyed by colour alone.
- **SHOULD** support light and dark themes.

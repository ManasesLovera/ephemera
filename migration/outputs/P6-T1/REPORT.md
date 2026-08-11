# P6-T1: RSS Validation — Full Slint App vs Pre-migration Tauri App

Date: 2026-08-11
Methodology: P0-T2 spike methodology applied to full merged Slint app
See: `migration/prompts/P0-T2-spike.md`, `migration/outputs/P6-T1/run_comparison.py`

## How Measurements Were Taken

### GUI Idle RSS
Each app was launched in release mode, given 5-10 seconds to settle (window
visible, event loop running), then VmRSS was read from `/proc/<pid>/status`.
The Tauri app's tree RSS sums the main process plus its WebKitGTK child
processes (WebKitNetworkProcess, WebKitWebProcess).

### Workload RSS
Neither app supports CLI-driven file loading, and this Wayland session has no
automation tools (wtype, ydotool, dotool). A standalone CLI harness
(`migration/outputs/P6-T1/measure/`) exercises `ephemera-core` directly —
the same library both apps share — performing:

1. Initialize AppState (DB connection, vault, ram store, cloud store)
2. Upload a 10 MB file to RAM
3. Persist to disk (with `fsync`)
4. Save to PostgreSQL
5. Flush RAM

VmRSS is read from `/proc/self/status` at each step.

The workload delta (added RSS from each operation) is identical for both apps
since they share the same `ephemera-core` crate. The GUI framework overhead
is additive and constant, captured in the idle measurement.

### Cloud Tier
No GCS key is present on this machine — the cloud store initializes offline
and contributes no network overhead. Cloud tier measurements are excluded.

## Raw Measurements

### Core Workload Harness (ephemera-core CLI)

| State | VmRSS (kB) | MiB | Delta from idle |
|-------|-----------|-----|-----------------|
| Process start (baseline) | 6,620 | 6.5 | — |
| After state init (idle) | 14,184 | 13.9 | — |
| After RAM upload (10 MB) | 34,660 | 33.8 | +20,476 kB |
| After disk persist (fsync) | 34,660 | 33.8 | +20,476 kB |
| After DB save | 55,316 | 54.0 | +41,132 kB |
| After RAM flush | 55,316 | 54.0 | — |

RAM store reported 10.0 MiB after upload — confirmed. Disk throughput:
227.27 MB/s (real fsync). DB save produced additional ~21 MB of RSS over
the RAM+disk state (DB connection buffers, query serialization, sqlx pool).

### Slint App (full app, idle)

| Metric | Value |
|--------|-------|
| Main process VmRSS | 126,084 kB (123.1 MiB) |
| Tree VmRSS | 129,110,016 bytes (123.1 MiB) |
| Process count | 1 |

### Tauri App (pre-migration, idle)

| Metric | Value |
|--------|-------|
| Main process VmRSS | 179,544 kB (175.3 MiB) |
| Tree VmRSS (3 processes) | 411,709,440 bytes (392.6 MiB) |
| Process count | 3 (main + 2 WebKitGTK children) |

Child processes:
- WebKitNetworkProcess: 57,508 kB
- WebKitWebProcess: 165,008 kB
- bwrap sandbox (glycin SVG loader): detected but not a direct child of the main PID

## Comparison

### Idle RSS

| | Slint | Tauri | Difference |
|--|-------|-------|------------|
| Main process | 126,084 kB | 179,544 kB | **+53,460 kB** (+42%) |
| Tree (all processes) | 126,084 kB | 401,935 kB | **+275,851 kB** (+219%) |

The Tauri app uses **3.2x more memory** at idle when counting the full process
tree. The WebKitGTK child processes alone account for ~222 MB — more than the
Slint app's entire footprint.

### Projected Workload RSS (Idle + Core Delta)

Since the core workload delta is shared, projected total RSS:

| Workload | Slint (est.) | Tauri main (est.) | Tauri tree (est.) |
|----------|-------------|-------------------|-------------------|
| Idle | 126,084 kB | 179,544 kB | 401,935 kB |
| + RAM load (10 MB) | 146,560 kB | 200,020 kB | 422,411 kB |
| + Disk persist | 146,560 kB | 200,020 kB | 422,411 kB |
| + DB save | 167,216 kB | 220,676 kB | 443,067 kB |

### Comparison with Phase 0 Spike

The P0-T2 spike (standalone Slint window, no app logic, 50 MiB buffer)
measured:

| | P0 Spike Slint | Full Slint App | Delta |
|--|---------------|---------------|-------|
| Idle main VmRSS | 92,428 kB | 126,084 kB | +33,656 kB |

The 34 MB difference is the full app's additional overhead: DB connection pool
(sqlx + tokio), vault index, reqwest HTTP client (for GCS), richer Slint UI
components, and the 4 Hz metrics sampler thread.

The P0 spike could not measure the Tauri post-allocation point (no Wayland
automation), so cross-app workload comparisons had no baseline.

## Key Findings

1. **The Slint app uses dramatically less memory**: 123 MiB vs 393 MiB tree
   RSS at idle. This confirms the Phase 0 spike's hypothesis — dropping
   WebKitGTK produces a substantial memory reduction, not just a marginal one.
   The full-app numbers are consistent with the spike's direction: the spike
   showed a 53% reduction in main-process RSS; the full app shows a 30%
   reduction in main-process RSS and a 69% reduction in tree RSS.

2. **The full Slint app is heavier than the spike**: 126 MB vs 92 MB idle.
   This is expected — the full app has a DB connection pool, HTTP client,
   vault index, metrics sampler, and richer UI. The overhead is real and
   measured, not estimated.

3. **Workload scaling is similar**: The core delta for a 10 MB RAM upload is
   ~20 MB (the 10 MB allocation + allocator overhead + resident page cost).
   This applies equally to both apps since they share `ephemera-core`.

4. **DB save adds significant RSS**: The DB save step adds ~21 MB over the
   RAM+disk state. This is from sqlx connection pool buffers, query
   serialization, and the BYTEA encoding. It doesn't drop after RAM flush
   because the Rust allocator retains the freed pages.

5. **No GCS measurement possible**: No service account key on this machine.
   The cloud store initializes offline. Cloud upload RSS would include
   reqwest HTTP buffers and JWT token exchange overhead.

6. **Tauri child process overhead is the dominant factor**: The WebKitGTK
   children (222 MB) outweigh any difference in main-process RSS (53 MB).
   Dropping Tauri eliminates these processes entirely.

## Reproduce

```bash
# Build Slint app
cargo build --release
# (from crates/ephemera-app/)

# Build measurement harness
cd migration/outputs/P6-T1/measure && cargo build --release

# Run harness for core deltas
./target/release/p6t1-measure 10485760

# Measure idle RSS manually
./crates/ephemera-app/target/release/ephemera-app &
PID=$!; sleep 8; grep VmRSS /proc/$PID/status; kill $PID

/tmp/opencode/tauri-pre/src-tauri/target/release/ephemera &
PID=$!; sleep 10; grep VmRSS /proc/$PID/status; kill $PID
```

## Limitations

- **No GUI workload automation**: Neither app accepts CLI flags for file
  loading. The workload deltas are measured from a standalone core harness
  and projected onto the idle baselines. This is honest but indirect.
- **Single measurement per state**: Without Wayland automation tools, each
  measurement requires a manual launch/kill cycle. Repeat runs would require
  scripted iteration (possible but adds noise from desktop session state).
- **No GCS tier**: Cloud upload RSS is untested.
- **Tauri app is from a worktree** (`/tmp/opencode/tauri-pre/`), checked out
  at the pre-migration commit (03e62d0). It was built previously; the build
  was not refreshed for this test.

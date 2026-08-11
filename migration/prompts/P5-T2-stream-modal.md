Disposable git worktree/branch — safe to write freely.

Port `src/components/StreamModal.tsx` to Slint — this is the RAM-bypass streaming
path's UI: a modal that reports measured peak memory for the stream-to-disk copy
against the buffered alternative. Read `src/components/StreamModal.tsx` and
`docs/07-streaming.md` in full before starting.

Hard invariant: the comparison this modal shows (streamed peak memory vs buffered
peak memory) must be built from real measurements taken during the actual copy, not
computed/estimated after the fact. If the current implementation already measures
correctly, preserve the measurement path exactly when porting — don't accidentally
replace a live measurement with a computed estimate because it's easier to wire up
in Slint's modal/dialog model.

Concrete plan (a prior attempt burned its whole budget reading context and never
wrote a file — don't repeat that; get to writing code quickly):

1. In `crates/ephemera-app/src/main.rs`, `on_stream_upload_to_disk`'s handler already
   calls `ephemera_core::stream_upload_to_disk(...)` but discards the returned
   `Ok(_report)` — that report is the real, already-measured data this modal needs.
   Stop discarding it: store it (e.g. a `Mutex<Option<StreamReport>>` slot on
   `ShellState`, mirroring how `metrics`/`db_status` are already held) and push its
   fields into new Slint properties, then flip a `show-stream-report: bool` property
   so the modal becomes visible.
2. In `app.slint`, add a `StreamReport` struct (bytes_total, elapsed_ms, chunk_size,
   buffered_equivalent_peak_bytes, max_concurrent_streaming, max_concurrent_buffered,
   rss_baseline_bytes, rss_peak_bytes, rss_avg_bytes, rss_peak_delta_bytes, file_name)
   and a small popup/overlay component rendering the same fields as
   `StreamModal.tsx` (peak vs buffered-equivalent stat pair, RSS baseline/peak/avg
   line, a close button wired to a `close-stream-report()` callback). A `Rectangle`
   covering the window with a centered card is enough — no need for Slint's
   `PopupWindow` unless it's simpler.
3. Wire the close callback in `main.rs` to clear `show-stream-report`.

Run `cargo check` and paste output at the end. If you're running low on remaining
turns/budget, prioritize landing a working (even visually rough) modal over further
research — a real file changed beats a longer investigation.

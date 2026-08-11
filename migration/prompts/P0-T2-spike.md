You are working in a disposable git worktree/branch of the `ephemera` repo — safe to
write freely here, this never touches `main` directly.

Goal: build the smallest possible Slint proof-of-concept that answers one question —
does dropping Tauri's WebKitGTK webview actually lower this app's RAM baseline?

Read `docs/02-architecture.md` and `docs/03-ui-and-visualization.md` first for the
real state model and the RAM tier's current behavior (`src-tauri/src/commands/ram.rs`,
`src/components/RamPane.tsx`, `src/components/Meter.tsx`, `src/components/StatTile.tsx`).

Build, under `spike-slint/`:

1. A Slint `.slint` window with a meter bar and a stat tile, bound to Rust properties.
2. A Rust binary that updates those properties from a timer at 4 Hz (matching the
   real dashboard's tick rate — see `docs/03-ui-and-visualization.md`), driven by the
   same kind of in-process buffer accounting as `ram.rs` (a real `Vec<u8>`/similar
   heap allocation you resize, not a fake number — the whole point of this app is that
   displayed numbers are measured, not simulated; keep that true here too).
3. A short `spike-slint/MEASURE.md` describing exactly how to reproduce an RSS
   measurement for this binary (e.g. `/proc/<pid>/status` VmRSS at idle and after
   loading a known-size buffer) and instructions to compare it against the equivalent
   measurement for the existing Tauri build.

Do not attempt to port anything else. This is a feasibility spike, not phase 1 of
the real port. Keep it small enough that a reviewer can read the whole diff in one
sitting.

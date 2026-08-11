You are working in a disposable git worktree/branch — safe to write freely, never
touches `main` directly.

Goal: extract the business logic currently living in `src-tauri/src/commands/{ram,
disk,stream,db,cloud,config}.rs` into a Tauri-free module (e.g. `src-tauri/src/core/`
or a new `ephemera-core` crate) — plain Rust functions and types with no
`tauri::State`, `tauri::Window`, `#[tauri::command]`, or IPC-shaped
serialization assumptions. The existing `commands/*.rs` files should become thin
wrappers that call into this core module, so the current Tauri build keeps working
unmodified while a future Slint shell can call the same core functions directly.

Hard invariants from `CLAUDE.md` that must survive this refactor byte-for-byte —
do not "clean up" or change these while extracting:

- RAM tier is a real in-process heap buffer, capped at 10 MB.
- Disk tier writes real files to the vault folder, capped at 20 MB, with `fsync` on
  every write — never skip this even if it looks redundant.
- The DB tier (Postgres, `BYTEA`, 100 MB cap) and GCS tier (100 MB UI cap) are
  one-way from RAM — nothing flows back to RAM.
- No file bytes are ever written outside the vault folder — no temp files, no
  caches.
- The streaming path (`stream.rs`) bypasses RAM entirely by design; keep that
  bypass intact, don't accidentally route it through the RAM buffer during
  extraction.

Read `docs/02-architecture.md` for the full state model before touching anything.
Run `cargo check` (and `cargo test` if tests exist) in the worktree before finishing,
and paste the output at the end of your response.

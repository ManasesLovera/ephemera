You are working in a disposable git worktree/branch — safe to write freely.

Goal: build the Slint application shell that replaces the Tauri+React shell,
wired directly to the Tauri-free core module produced in `migration/P1-T1`
(merge that branch into yours first, or rebase on top of it — check
`git log --all --oneline` for the `migration/P1-T1` branch).

Scope for this task only:

- A top-level Slint window with placeholder panels for RAM/Disk/Sink (DB+GCS), no
  real content yet — those come in Phase 3.
- A Rust-side property/state model that mirrors what `src/store/` currently holds,
  but bound directly to Slint properties/callbacks instead of Tauri's
  `invoke()`/event-emit IPC. There is no IPC boundary anymore — the UI and core run
  in one process, one address space.
- Confirm and preserve this project rule: the UI layer must never hold a second
  authoritative copy of file bytes. In the old Tauri app this meant "the frontend
  only holds metadata." In Slint it means the same thing — Slint properties should
  carry sizes/names/status, never the actual byte buffer content, even though
  there's no serialization boundary forcing that separation anymore. Don't let the
  removal of IPC accidentally erase this discipline.

Read `docs/02-architecture.md` (state model) and `docs/03-ui-and-visualization.md`
(layout) before starting. Run `cargo check` and paste the output at the end.

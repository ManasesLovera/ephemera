# Ephemera — instructions

## Read before doing anything

This is a **working app**, natively rendered with [Slint](https://slint.dev) (migrated
from an earlier Tauri + React shell — see [`migration/PLAN.md`](migration/PLAN.md) and
[`migration/tasks.json`](migration/tasks.json) for how, and
[`migration/outputs/P6-T1/REPORT.md`](migration/outputs/P6-T1/REPORT.md) for the
measured RSS win). RAM, disk, database (Postgres), and cloud (GCS) tiers are all
implemented and tested, CI is green. Start with
[`docs/10-implementation-status.md`](docs/10-implementation-status.md) — it's the
honest diff between the original spec and what's actually built, and lists the known
gaps to pick up next. Then the rest of `docs/` for the full spec:

1. `docs/00-vision.md` — what this is and why
2. `docs/01-requirements.md` — MUST/SHOULD/COULD, the hard limits, the tier graph
3. `docs/02-architecture.md` — Rust state model, IPC surface, the known gotchas
4. `docs/03-ui-and-visualization.md` — layout, drag & drop, every chart
5. `docs/04-tech-stack.md` — chosen libraries, verified machine prereqs
6. `docs/05-teaching-notes.md` — the concepts and which UI element teaches each
7. `docs/06-open-questions.md` — remaining decisions
8. `docs/07-streaming.md` — the RAM-bypass streaming path
9. `docs/08-database-tier.md` — Postgres tier
10. `docs/09-gcs-tier.md` — cloud tier + GCP setup guide

## Git & release workflow — read before touching git

- **Do not push after every requested change.** Commit and push only when the user
  explicitly asks for it in that turn — a past instruction to commit/push does not
  carry forward to later, unrelated changes; ask again (or wait to be asked) each time.
- **Commits happen only on explicit user request.** Making a change is not, by itself,
  a request to commit it. Batch unpushed work rather than committing reflexively after
  each edit.
- **Tags and releases happen only on an explicit trigger** — phrasing like "create a
  new release" or "create a new tag". Never tag or release as a side effect of
  finishing a feature, even a big one.
- **When asked to create a release:**
  1. Check the current/last tag (`git tag --sort=-v:refname | head -1`, or
     `gh release list --limit 1`) and decide the next version.
  2. Bump the version together in `crates/ephemera-app/Cargo.toml` and
     `crates/ephemera-core/Cargo.toml` — then run `cargo check` in both crates once to
     refresh their `Cargo.lock`s before committing.
  3. **Release notes must be a full changelog of every change since the last tag**,
     not just the latest commit — build it from `git log <last-tag>..HEAD --oneline`
     (grouped by type if there's enough to group).
  4. Reference [`DOWNLOAD.md`](DOWNLOAD.md) in the release notes for how to get or
     build the binaries.
  5. **Releases are always regular releases, never pre-releases.** `release.yml` sets
     `prerelease: false`; a manual `gh release create` must not pass `--prerelease`.
  6. Push the tag (`git push origin vX.Y.Z`) to trigger `release.yml`.
- Commits that update these instructions themselves (this file) use the `chore:`
  conventional-commit type.

## What this app is

A native [Slint](https://slint.dev) desktop app (`crates/ephemera-app`, over the
Tauri-free `crates/ephemera-core` logic crate) that teaches the storage hierarchy: RAM,
disk, database, and cloud. Files uploaded live **only** in the Rust process heap (10 MB
cap) until explicitly carried to disk (20 MB cap, real `fsync`), a Postgres database
(100 MB cap, `BYTEA`, via `docker compose`), or a GCS bucket (100 MB UI cap) — one-way,
never back to RAM. A second "stream to disk" path bypasses RAM entirely, chunk-copying
with a completion report comparing measured peak memory against the buffered
alternative. A live 4 Hz dashboard tracks per-tier usage and the process's actual
resident memory. UI and core run in one process, one address space — no IPC, no
serialization boundary (a change from the earlier Tauri build; see
`docs/02-architecture.md`).

## Rules specific to this project

- **The metaphor must stay real.** The RAM store is a real in-memory buffer; the disk
  store is real files in a real folder. If something must be simulated (e.g. the disk
  latency throttle), it is labelled as simulated in the UI. Never present a fabricated
  number as measured.
- **Never write file bytes anywhere except the vault folder.** No temp files, no caches,
  no autosave. A stray temp file would make the app's central claim false.
- **The UI layer never holds authoritative file bytes** — metadata only. Even with no
  IPC boundary forcing the separation anymore, Slint properties carry sizes/names/
  status, never byte content — caching contents there would make the usage numbers lie.
- **`fsync` on every disk write.** Without it, a write returns at page-cache speed and
  the app teaches the opposite of the truth.
- **No dual-axis charts.** RAM store bytes and process RSS go in two stacked charts
  sharing an x-axis, never one plot with two y-scales. See `docs/03`.
- Before shipping any chart, load the `dataviz` skill and run its palette validator for
  both light and dark modes.
- **Known Slint 1.17 limitation:** its `DataTransfer`/`DropArea` API carries only
  images, plain text, and same-process payloads — no file paths — so real OS-level
  drag-and-drop of files from outside the window (e.g. from a file manager) cannot be
  implemented on this Slint version. Don't silently claim it works in UI copy; the
  click-to-browse file picker (`rfd`) is the working equivalent.

## Environment notes

Verified 2026-08-11: Rust 1.97.1, Slint 1.17.1. No Node/pnpm/webkit2gtk needed anymore
— the Slint build only needs the Linux prereqs listed in
[`DOWNLOAD.md`](DOWNLOAD.md#linux) (fontconfig, X11/Wayland, GL, GTK dev headers).

This machine is **GNOME on Wayland with no Xorg**, confirmed working: the release
binary launches and stays alive with no errors (no screenshot tool was available in
this environment to also verify pixel-level layout — see
`docs/10-implementation-status.md`).

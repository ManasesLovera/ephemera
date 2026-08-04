# Ephemera — instructions

## Read before doing anything

This is a **working app** — RAM, disk, database (Postgres), and cloud (GCS) tiers are
all implemented and tested, CI is green. Start with
[`docs/10-implementation-status.md`](docs/10-implementation-status.md) — it's the
honest diff between the original spec and what's actually built, and lists the known
gaps to pick up next (screenshot/visual verification is the top one). Then the rest of
`docs/` for the full spec:

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
  2. Bump the version in all three places together — `package.json`,
     `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` — then run `cargo check`
     once to refresh `Cargo.lock` before committing.
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

A Tauri 2 + React desktop app that teaches the storage hierarchy: RAM, disk, database,
and cloud. Files uploaded live **only** in the Rust process heap (10 MB cap) until
explicitly carried to disk (20 MB cap, real `fsync`), a Postgres database (100 MB cap,
`BYTEA`, via `docker compose`), or a GCS bucket (100 MB UI cap) — one-way, never back to
RAM. A second "stream to disk" path bypasses RAM entirely, chunk-copying with a
completion report comparing measured peak memory against the buffered alternative. A
live 4 Hz dashboard tracks per-tier usage and the process's actual resident memory.

## Rules specific to this project

- **The metaphor must stay real.** The RAM store is a real in-memory buffer; the disk
  store is real files in a real folder. If something must be simulated (e.g. the disk
  latency throttle), it is labelled as simulated in the UI. Never present a fabricated
  number as measured.
- **Never write file bytes anywhere except the vault folder.** No temp files, no caches,
  no autosave. A stray temp file would make the app's central claim false.
- **The frontend never holds authoritative file bytes** — metadata only. Webview memory
  is also RAM, and caching contents there would make the usage numbers lie.
- **`fsync` on every disk write.** Without it, a write returns at page-cache speed and
  the app teaches the opposite of the truth.
- **No dual-axis charts.** RAM store bytes and process RSS go in two stacked charts
  sharing an x-axis, never one plot with two y-scales. See `docs/03`.
- Before shipping any chart, load the `dataviz` skill and run its palette validator for
  both light and dark modes.

## Environment notes

Verified 2026-08-04: Rust 1.97.1, Node 24.13.0, pnpm 11.2.2, webkit2gtk-4.1 2.52.3 —
all Tauri Linux prereqs present. Only `tauri-cli` needs installing.

This machine is **GNOME on Wayland with no Xorg**. If the Tauri window renders blank,
try `WEBKIT_DISABLE_DMABUF_RENDERER=1` before assuming an app bug.

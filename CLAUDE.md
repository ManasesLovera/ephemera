# Ephemera — instructions

## Read before doing anything

This project is **in design phase — no code exists yet.** The complete specification is
in [`docs/`](docs/) and was written on 2026-08-04. Read it in order before proposing or
writing anything:

1. `docs/00-vision.md` — what this is and why
2. `docs/01-requirements.md` — MUST/SHOULD/COULD, the hard limits
3. `docs/02-architecture.md` — Rust state model, IPC surface, the known gotchas
4. `docs/03-ui-and-visualization.md` — layout, drag & drop, every chart
5. `docs/04-tech-stack.md` — chosen libraries, verified machine prereqs
6. `docs/05-teaching-notes.md` — the concepts and which UI element teaches each
7. `docs/06-open-questions.md` — what still needs deciding, plus the build order

`docs/06-open-questions.md` ends with a staged build order. Start there.

## What this app is

A Tauri 2 + React desktop app that teaches RAM vs. disk. Files uploaded live **only** in
the Rust process heap, capped at 10 MB, and are lost when the app exits. Carrying a file
to a configurable "vault" folder on disk (capped at 20 MB) is an explicit user action.
A live dashboard shows per-file usage of both stores, real-time memory during transfers,
and the process's actual resident memory.

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

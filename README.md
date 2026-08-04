# Ephemera

> A desktop app that teaches the difference between **RAM** and **disk** by making you
> feel it. Files you upload live *only* in RAM. They disappear when the app closes —
> unless you deliberately carry them across to disk.

**Status:** design phase. No code written yet. All specification lives in [`docs/`](docs/).

---

## Read this first

This project was specified in a single conversation on **2026-08-04** and the docs below
capture everything decided in it. Nothing here has been implemented — the next session
should start by reading the docs in order, then produce an implementation plan.

| Doc | What it answers |
| --- | --- |
| [`docs/00-vision.md`](docs/00-vision.md) | What Ephemera is and why it exists |
| [`docs/01-requirements.md`](docs/01-requirements.md) | Exactly what it must do; the hard limits |
| [`docs/02-architecture.md`](docs/02-architecture.md) | Rust backend, state model, IPC surface, events |
| [`docs/03-ui-and-visualization.md`](docs/03-ui-and-visualization.md) | Two-pane layout, drag & drop, every chart |
| [`docs/04-tech-stack.md`](docs/04-tech-stack.md) | Chosen libraries with rationale + verified machine prereqs |
| [`docs/05-teaching-notes.md`](docs/05-teaching-notes.md) | The CS concepts, and which UI element teaches each |
| [`docs/06-open-questions.md`](docs/06-open-questions.md) | Decisions still needed before/during build |

## The idea in one paragraph

Ephemera looks like a stripped-down Google Drive with two panes. The left pane is
**RAM** — drop files in and they are held as bytes in the Rust process heap, capped at
**10 MB**, and they are gone the moment the app exits. The right pane is **Disk** — a
single folder you configure, capped at **20 MB**, whose contents survive anything. The
only way a file crosses from left to right is an explicit action. Around those two panes
sits a live dashboard: per-store usage meters broken down by file, a real-time memory
graph that moves while you upload, transfer throughput for each direction, and the
process's *actual* resident memory so you can see how much the app itself costs beyond
the bytes you stored.

## Name

*Ephemera* — things that exist for only a short time. The RAM pane is the ephemera; the
whole app is about what it takes to make something last.

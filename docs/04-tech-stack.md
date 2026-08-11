# 04 — Tech stack

> [!note]
> This document originally described the pre-migration Tauri 2 + React stack. The app
> now uses a native [Slint](https://slint.dev) UI over the same `ephemera-core` logic —
> see [`migration/PLAN.md`](../migration/PLAN.md) for how the migration was run.

## Machine prerequisites — verified 2026-08-11

| Requirement | Found | Status |
| --- | --- | --- |
| OS | Ubuntu 26.04 LTS | ok |
| `rustc` / `cargo` | 1.97.1 | ok |
| Slint | 1.17.1 | ok (pulled as a crate dependency, nothing to install separately) |
| `docker` | 29.6.0, daemon active, user in `docker` group | ok |
| `docker compose` | v5.1.4 | ok |
| `gcloud` SDK | 565.0.0, ADC present & valid | ok |
| Linux build deps (fontconfig, X11/Wayland, GL, GTK dev headers) | present | see [`DOWNLOAD.md`](../DOWNLOAD.md#linux) for the exact package list |

No Node/pnpm/webkit2gtk/librsvg required anymore — those were Tauri+React-specific.

Re-verify before building:

```bash
rustc --version && cargo --version
```

> [!note]
> This machine runs **GNOME on Wayland with no Xorg**. Confirmed working: the Slint
> release binary launches and stays alive with no errors under Wayland. Unlike the old
> WebKitGTK build, Slint has no known Wayland-specific rendering flags needed here.

## Rust crates

| Crate | Purpose | Notes |
| --- | --- | --- |
| `slint` v1.17 | UI toolkit — windowing, rendering, the `.slint` markup language | `crates/ephemera-app`; compiles `ui/app.slint` into Rust structs via `slint-build` |
| `rfd` | Native folder & file pickers | Replaces `tauri-plugin-dialog` |
| `serde` / `serde_json` | Struct (de)serialization for structs shared with the DB/cloud tiers | No longer an IPC boundary — just ordinary data modeling now |
| `sysinfo` | Process RSS sampling | Must walk the process tree — see [`02-architecture.md`](02-architecture.md) |
| `uuid` v4 | File ids | |
| `indexmap` | Stable insertion order for stores | |
| `thiserror` | Typed `AppError` | |
| `tokio` | Async runtime | Chunked I/O, throttle sleeps, DB/cloud async calls, the 5 s poller |
| `mime_guess` | MIME sniffing from filenames | |
| `sqlx` (features: `postgres`, `runtime-tokio-rustls`, `uuid`, `chrono`, `macros`, `migrate`) | Database tier | Async Postgres client + built-in migrations; see [`08-database-tier.md`](08-database-tier.md) |
| `reqwest` + `jsonwebtoken` | Cloud tier | GCS REST API client with a JWT service-account key; see [`09-gcs-tier.md`](09-gcs-tier.md) |
| `dotenvy` | Load `DATABASE_URL` / GCS config from `.env` | Dev-only convenience; both `.env` and the GCS key are gitignored. Resolved relative to the executable's own directory (walking upward), not the process's CWD — see the `find_upwards` helper in `crates/ephemera-app/src/main.rs` |
| `chrono` | Timestamp formatting | |

Deliberately **not** used: `notify` (filesystem watching). Rescan on focus is simpler
and sufficient; a watcher adds a background source of state changes that would complicate
the accounting for very little benefit. Revisit only if live external-change detection
becomes a wanted demo.

## UI — Slint

Slint compiles a `.slint` markup file (`crates/ephemera-app/ui/app.slint`) into
generated Rust structs/callbacks via `slint-build` (see `build.rs`), giving native
rendering with no browser engine, no JS runtime, and no serialization boundary between
UI and business logic.

### Why Slint over the alternatives considered

The migration's goal was cutting the WebKitGTK RAM baseline while keeping the app a
single native binary. Slint won out over egui/iced primarily because its declarative
markup (closer to CSS/QML than immediate-mode Rust) made porting the existing
React component tree tractable — see `migration/prompts/P0-T1-inventory.md` for the
original component-by-component inventory this was based on.

### Known Slint 1.17 limitations hit during the port

- **No charting library, and `Path` rejects `for` loops.** The rolling 60 s history
  sparklines (`Instruments` in `app.slint`) are hand-rolled as bar-style
  `for point in history: Rectangle { ... }` sparklines instead of an SVG line chart —
  the same segmented-bar technique `Meter` already used. See
  [`02-architecture.md`](02-architecture.md#dashboard-rendering-note).
- **`DataTransfer` carries no file paths.** Real OS-level drag-and-drop of files from
  outside the window (e.g. a file manager) isn't implementable on this Slint version —
  only images, plain text, and same-process payloads cross a `DropArea`. The
  click-to-browse file picker (`rfd`) is the working equivalent. See
  [`02-architecture.md`](02-architecture.md#getting-file-bytes-into-the-store).

### On charts vs. hand-rolled shapes

There's no equivalent of Recharts/visx for Slint — every chart in `03-ui-and-visualization.md`
(meters, stat tiles, sparklines) is drawn with plain `Rectangle`/`Text` elements bound
to Slint properties. This is actually closer to the original design intent than the
React version's Recharts dependency: the design called for 2px inter-segment surface
gaps, rounded outer ends only, and a cap marker — simple shapes that don't need a
charting library's abstraction, just consistently applied geometry.

### Rendering budget

The metrics stream is 4 Hz. `ShellState::push_metrics` (`model.rs`) mutates a
persistent `VecModel` in place (push/pop) rather than rebuilding it every tick — a
naive rebuild would itself churn the heap this app exists to measure honestly. See the
doc comment there for the full reasoning; this was a real review finding during the
migration (Phase 4), not a hypothetical concern.

## Palette and design tokens

Take the categorical slots, sequential ramp, status colors, surfaces, and ink tokens
from the dataviz reference palette, and define them as color constants in
`app.slint` (Slint has no CSS-custom-property equivalent, so these live as component
properties/constants rather than a shared token file).

**Before shipping any chart, run the palette validator** for both light and dark modes
against the actual surfaces used. Do not eyeball colorblind-safety. Slint's own
dark-mode support is more limited than a CSS media query — verify how the app currently
handles theme switching before assuming both modes are covered.

## Project layout

```text
ephemera/
├── docs/                        # this specification
├── docker-compose.yml           # postgres, local dev only
├── .gitignore                   # excludes crates/ephemera-app/.env, gcs-key.json
├── migration/                   # Tauri → Slint migration plan, tooling, and record
├── crates/
│   ├── ephemera-core/           # Tauri-free logic crate — no UI dependency at all
│   │   ├── src/
│   │   │   ├── lib.rs           # AppState, config, metrics
│   │   │   ├── config_file.rs   # vault-path persistence in the OS config dir
│   │   │   ├── ram_store.rs, vault.rs, db_store.rs, cloud_store.rs
│   │   │   ├── stream.rs        # chunked read/write, StreamReport
│   │   │   ├── state.rs         # AppState, sampler spawning
│   │   │   ├── types.rs, error.rs
│   │   │   └── migrations/      # sqlx migrations (the files table from docs/08)
│   │   └── tests/               # unit + real Postgres/GCS integration tests
│   └── ephemera-app/             # the Slint binary — the only UI, standalone workspace
│       ├── .env                 # DATABASE_URL — gitignored, dev-only
│       ├── gcs-key.json         # service-account key — gitignored, never commit
│       ├── ui/app.slint         # the entire UI: types, components, the shell window
│       ├── src/
│       │   ├── main.rs          # wiring: AppState construction, callbacks, sampler/poller
│       │   └── model.rs         # ShellState — UI-layer state, core→Slint projection
│       └── build.rs             # slint-build codegen
└── README.md
```

> [!note]
> `crates/ephemera-app/Cargo.toml` declares its own empty `[workspace]` table
> deliberately, so it never joins a parent workspace — see the comment there. This
> matters for anyone running `cargo` commands against it: use
> `cargo build --manifest-path crates/ephemera-app/Cargo.toml` or `cd` into the crate
> first, not `cargo build -p ephemera-app` from the repo root.

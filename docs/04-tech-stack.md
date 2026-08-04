# 04 — Tech stack

## Machine prerequisites — verified 2026-08-04

Checked on this machine. Everything Tauri 2 needs to build on Linux is already present.

| Requirement | Found | Status |
| --- | --- | --- |
| OS | Ubuntu 26.04 LTS | ok |
| `rustc` | 1.97.1 | ok |
| `cargo` | 1.97.1 | ok |
| `node` | v24.13.0 | ok |
| `pnpm` | 11.2.2 | ok |
| `bun` | 1.3.14 | available |
| `webkit2gtk-4.1` | 2.52.3 | ok |
| `javascriptcoregtk-4.1` | 2.52.3 | ok |
| `libsoup-3.0` | 3.6.6 | ok |
| `gtk+-3.0` | 3.24.52 | ok |
| `librsvg-2.0` | 2.61.3 | ok |
| `glib-2.0` | 2.88.0 | ok |
| `openssl` | 3.5.5 | ok |
| `cc` | gcc 15.2.0 | ok |
| `docker` | 29.6.0, daemon active, user in `docker` group | ok |
| `docker compose` | v5.1.4 | ok |
| `gcloud` SDK | 565.0.0, ADC present & valid | ok |
| **Tauri CLI** | **not installed** | **install step needed** |
| **Postgres (port 5432)** | free, not yet started | `docker compose up -d` once `docker-compose.yml` exists |
| **GCS bucket / service account** | not yet created | see [`09-gcs-tier.md`](09-gcs-tier.md) — awaiting go-ahead to create billable-but-free-tier resources |

Only missing piece:

```bash
cargo install tauri-cli --version "^2"
# or use the JS CLI via the scaffold: pnpm create tauri-app
```

Re-verify before building:

```bash
rustc --version && node --version && pnpm --version && \
pkg-config --modversion webkit2gtk-4.1 libsoup-3.0 librsvg-2.0
```

> [!note]
> This machine runs **GNOME on Wayland with no Xorg** (see the general memory
> `wayland-only-machine`). WebKitGTK under Wayland occasionally needs
> `WEBKIT_DISABLE_DMABUF_RENDERER=1` or `WEBKIT_DISABLE_COMPOSITING_MODE=1` if the
> window renders blank. Try those first before assuming an app bug.

## Backend — Rust

| Crate | Purpose | Notes |
| --- | --- | --- |
| `tauri` v2 | Shell, IPC, windowing | |
| `tauri-plugin-dialog` | Native folder & file pickers | |
| `tauri-plugin-fs` | Filesystem access | **Scope permissions to the vault only** |
| `tauri-plugin-opener` | Reveal vault in file manager | |
| `tauri-plugin-store` | Persist config (vault path, prefs) | Store in config dir, not the vault |
| `serde` / `serde_json` | IPC serialisation | |
| `sysinfo` | Process RSS sampling | Must walk the process tree — see architecture |
| `uuid` v4 | File ids | |
| `indexmap` | Stable insertion order for stores | |
| `thiserror` | Typed `AppError` for structured IPC errors | |
| `tokio` | Async runtime (Tauri brings it) | Chunked I/O, throttle sleeps |
| `infer` *(or `mime_guess`)* | MIME sniffing from bytes | Optional; nicer file icons |
| `sqlx` (features: `postgres`, `runtime-tokio-rustls`, `uuid`, `chrono`, `macros`) | Database tier | Async Postgres client + built-in migrations; see [`08-database-tier.md`](08-database-tier.md) |
| `google-cloud-storage` | Cloud tier | GCS client reading a service-account key; see [`09-gcs-tier.md`](09-gcs-tier.md) |
| `dotenvy` | Load `DATABASE_URL` / GCS config from `.env` | Dev-only convenience; both `.env` and the GCS key are gitignored |

Deliberately **not** used: `notify` (filesystem watching). Rescan on focus is simpler
and sufficient; a watcher adds a background source of state changes that would complicate
the accounting for very little benefit. Revisit only if live external-change detection
becomes a wanted demo.

## Frontend

| Choice | Why this one |
| --- | --- |
| **React 19 + TypeScript** | Requested. Types matter here because the IPC contract is the app's spine. |
| **Vite** | Tauri's default; instant HMR. |
| **Tailwind CSS v4** | CSS-first config suits mapping the viz palette to design tokens once. |
| **shadcn/ui** (Radix) | Copy-in components, accessible primitives, no runtime lock-in. Good dialogs, tooltips, drawer. |
| **dnd-kit** | The modern React DnD library — pointer-based, accessible (keyboard draggable), no HTML5 DnD dependency, so it coexists with Tauri's native drop handling. `react-beautiful-dnd` is deprecated; do not use it. |
| **Motion** (`motion/react`) | Successor to Framer Motion. Layout animations make the card-crossing transition nearly free. |
| **Zustand** | The metrics store updates at 4 Hz; Zustand's selector subscriptions keep that from re-rendering the whole tree. Context would. |
| **Recharts** | Composable React charting, adequate for 4 Hz with a 240-point window. |

### On Recharts vs. hand-rolled SVG

Use **Recharts for the time-series charts only**. The meters, stat tiles, and sparklines
should be **hand-written SVG/CSS**: they are simple shapes, and the design in
[`03-ui-and-visualization.md`](03-ui-and-visualization.md) calls for 2px inter-segment
surface gaps, 4px rounded outer ends only, ghost preview segments during drag, and a cap
marker. Fighting a charting library's defaults to achieve those costs more than drawing
two `<rect>` elements.

If Recharts proves heavy at 4 Hz, **visx** (low-level d3 primitives, React-native) is
the fallback. Do not reach for raw d3 with React — the two disagree about who owns
the DOM.

### Rendering budget

The metrics stream is 4 Hz. Chart re-renders must be isolated to the chart components
via Zustand selectors; the file lists and panes must not re-render on a metrics tick.
Verify with React DevTools Profiler once the dashboard is in — a dropped frame during
the upload animation would undermine the "watch it happen live" premise.

## Palette and design tokens

Take the categorical slots, sequential ramp, status colors, surfaces, and ink tokens
from the dataviz reference palette, and define them **once** as CSS custom properties
consumed by both Tailwind and the SVG charts. Light and dark values must be declared
under both `@media (prefers-color-scheme: dark)` and a `[data-theme]` scope so the app's
own theme toggle wins in both directions.

**Before shipping any chart, run the palette validator** for both modes against the
actual surfaces used. Do not eyeball colorblind-safety.

## Project layout

```text
ephemera/
├── docs/                    # this specification
├── docker-compose.yml       # postgres, local dev only
├── .gitignore                # excludes src-tauri/.env, src-tauri/gcs-key.json
├── src/                     # React frontend
│   ├── components/
│   │   ├── panes/           # RamPane, DiskPane, FileCard, DropZone
│   │   ├── sinks/           # DbPanel, CloudPanel — compact one-way destinations
│   │   ├── viz/             # StoreMeter, TimeSeries, ThroughputBars, StatTile, TierMap
│   │   └── ui/              # shadcn components
│   ├── lib/
│   │   ├── ipc.ts           # typed wrappers over invoke() — single source of truth
│   │   ├── format.ts        # byte and duration formatting
│   │   └── colors.ts        # FileId → categorical slot assignment
│   ├── store/               # Zustand stores (files, metrics, config)
│   └── types.ts             # mirrors Rust types across the IPC boundary
├── src-tauri/
│   ├── .env                 # DATABASE_URL — gitignored, dev-only
│   ├── gcs-key.json         # service-account key — gitignored, never commit
│   ├── migrations/          # sqlx migrations (the files table from docs/08)
│   ├── src/
│   │   ├── main.rs
│   │   ├── state.rs         # AppState, RamStore, DiskIndex, DbStore, CloudStore
│   │   ├── commands/        # one module per command group (ram, disk, db, cloud, stream)
│   │   ├── metrics.rs       # sysinfo sampler thread
│   │   ├── vault.rs         # path safety, scanning, sidecar index
│   │   ├── stream.rs        # chunked read/write, StreamReport
│   │   └── error.rs         # AppError
│   ├── capabilities/        # fs scope limited to the vault
│   └── tauri.conf.json
└── README.md
```

> [!tip]
> Keep `src/types.ts` and the Rust `Serialize` structs in sync by hand at first — the
> surface is small. If it starts drifting, add `ts-rs` to generate the TypeScript from
> the Rust types rather than maintaining two copies.

## Scaffolding command

```bash
cd ~/dev/ephemera
pnpm create tauri-app . --template react-ts --manager pnpm
```

Scaffold **into the existing folder** so `docs/` and `README.md` are preserved — check
the CLI's behaviour on a non-empty directory first, and if it refuses, scaffold to a
temp path and move the generated files in.

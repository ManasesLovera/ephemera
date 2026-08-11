You are auditing the `ephemera` repo (Tauri 2 + React app teaching the storage
hierarchy: RAM/disk/Postgres/GCS) ahead of a migration to Slint. This is a read-only
research task — do not edit any files.

Read:

- `src-tauri/src/commands/ram.rs`
- `src-tauri/src/commands/disk.rs`
- `src-tauri/src/commands/stream.rs`
- `src-tauri/src/commands/db.rs`
- `src-tauri/src/commands/cloud.rs`
- `src-tauri/src/commands/config.rs`
- `src/components/DiskPane.tsx`
- `src/components/FileCard.tsx`
- `src/components/Instruments.tsx`
- `src/components/Meter.tsx`
- `src/components/RamPane.tsx`
- `src/components/SinkPanel.tsx`
- `src/components/StatTile.tsx`
- `src/components/StreamModal.tsx`
- `src/store/` (whatever state management is in use)

Produce a markdown table with one row per Tauri command: command name, input args,
return type, which frontend component(s) call it, and whether it's polled (dashboard
tick) or event-driven (user action). Then a second table per frontend component:
name, props, which Tauri commands/events it depends on, and any DOM/CSS feature it
relies on that has no direct Slint equivalent (e.g. native HTML5 drag-and-drop, CSS
grid, a charting library).

End with a short "risk list" — the 3-5 things most likely to blow up effort estimates
during the Slint port (e.g. "Instruments.tsx assumes an SVG-based charting lib with no
Slint equivalent").

Output only the markdown report to stdout.

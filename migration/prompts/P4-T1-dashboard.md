Disposable git worktree/branch — safe to write freely. This is the highest-risk
phase: Slint has no built-in charting library, so `src/components/Instruments.tsx`,
`Meter.tsx`, and `StatTile.tsx`'s dashboard behavior must be hand-rolled with Slint's
`Path`/canvas-style drawing primitives.

Read those three files plus `docs/03-ui-and-visualization.md`'s full chart section
before starting. The dashboard ticks at 4 Hz — read `docs/07-streaming.md` too if the
streaming path feeds any of these charts.

Hard invariant from `CLAUDE.md`, load-bearing for this task specifically: **no
dual-axis charts**. RAM store bytes and process RSS must be two separate stacked
charts sharing an x-axis, never one plot with two y-scales. If the current React
version already does this correctly, preserve that layout exactly — don't
"simplify" it back into a dual-axis chart because that's easier to hand-roll in
Slint.

If you hit a wall on any specific chart type, stop and describe the blocker in your
response rather than shipping a fabricated/placeholder number in its place — a fake
number in this app is worse than a missing chart.

Implementation note (read this before reaching for `Path`): Slint's `Path` element
does not allow a `for` loop inside it, so a dynamic SVG-command line chart isn't
viable here — a prior attempt at this task burned its whole budget on that dead end
and shipped nothing. Don't go there. Instead, render each stacked time-series chart
(RAM store bytes, process RSS) the same way `Meter`'s segmented bar already works in
this codebase: a `HorizontalLayout` of thin `Rectangle` bars, one per history point,
each bar's `height` bound to `parent.height * (value / max)`. That's a real bar-style
sparkline built entirely from `for point in history: Rectangle { ... }`, no `Path`
needed, and it matches the existing component style in `app.slint`. `ShellState`
already keeps a 240-point ring buffer (`history`/`MetricPoint`) in `model.rs` from
Phase 2 — project that into a `[HistoryPoint]`-shaped Slint property (ram + rss
values) rather than inventing new plumbing.

Run `cargo check` and paste output at the end. If you're running low on remaining
turns/budget, prioritize landing a working (even if visually rough) bar-sparkline
over further research — a real file changed beats a longer investigation.

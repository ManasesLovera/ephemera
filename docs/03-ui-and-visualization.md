# 03 — UI and visualization

## Layout

```text
┌──────────────────────────────────────────────────────────────────────┐
│  Ephemera        vault: ~/.local/share/ephemera/vault  [⚙] [◐]       │  chrome
├──────────────────────────────────────────────────────────────────────┤
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐         │
│  │ RAM USED   │ │ DISK USED  │ │ APP MEMORY │ │ FILES      │         │  KPI row
│  │ 4.2 MB     │ │ 11.8 MB    │ │ ~187 MB    │ │ 3 / 5      │         │  (stat tiles)
│  │ of 10 MB   │ │ of 20 MB   │ │ process ≈  │ │ ram / disk │         │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘         │
├───────────────────────────────────┬──────────────────────────────────┤
│  RAM · volatile                   │  DISK · persistent               │
│  ▓▓▓▓▓▓▒▒▒░░░░░░░░░░  4.2/10 MB   │  ▓▓▓▓▓▓▓▓▓▓▒▒▒░░░░  11.8/20 MB   │  segmented
│  ┌─────────────────────────────┐  │  ┌────────────────────────────┐  │  meters
│  │ ▪ photo.jpg  2.1MB [DB][☁]⇥ │  │  │ ▪ report.pdf 6.0MB[DB][☁]  │  │
│  │ ▪ notes.txt  0.1MB [DB][☁]⇥ │  │  │ ▪ audio.wav  5.8MB[DB][☁]  │  │  file lists
│  │ ▪ audio.wav  2.0MB [DB][☁]⇥ │  │  │                            │  │  (each card:
│  │                             │  │  │                            │  │  save-to-DB /
│  │   drop files here           │  │  │   drag from RAM to keep    │  │  save-to-cloud
│  └─────────────────────────────┘  │  └────────────────────────────┘  │  buttons)
│  [ browse… ] [ stream to disk… ]  │  [ open folder ]  [ change… ]    │
│  [ pull the plug ]                │                                  │
├───────────────────────────────────┴──────────────────────────────────┤
│  DATABASE · postgres (docker)      │  CLOUD · gcs bucket              │  compact
│  ▓▓▓▓▓▒░░░░░░░░  32/100 MB logical │  ▓▓░░░░░░░░░░░  8/100 MB          │  sink
│  38 MB physical (incl. TOAST)      │  ephemera-vault-alterna           │  panels
├───────────────────────────────────────────────────────────────────────┤
│  ▾ Instruments                                                        │  drawer
│    (time series · throughput ladder · tier map · file table)          │  collapsible
└──────────────────────────────────────────────────────────────────────┘
```

The two big panes — RAM and Disk — are still the app's primary surface, since they carry
the core lesson and the drag gesture. Database and Cloud are drawn as **compact sink
panels** beneath them, not full panes: they are one-way destinations reached by a button
on any RAM/disk card, never a source you drag *from* (see the tier graph in
[`01-requirements.md`](01-requirements.md#moving-files-and-the-tier-graph)). Giving them
equal visual weight to RAM/Disk would blur that asymmetry.

The **Instruments** drawer holds everything that explains the panes, collapsed by
default so the first impression is "a file app", expandable when the user starts asking
why. Left/right for RAM/Disk matters: horizontal movement gives the drag gesture a
direction, and "carrying a file across" is the metaphor.

Asymmetry is deliberate throughout — meter track lengths are drawn on a **shared
bytes-per-pixel scale** across all four tiers (RAM shortest, Disk 2×, Database and Cloud
10×), so relative capacity is visible without reading a number. The panes/panels
themselves stay comparably sized for layout sanity; only the meter *tracks* scale.

## Drag and drop — two different mechanisms

These are separate systems and conflating them is the main integration risk.

| Gesture | Mechanism | Library |
| --- | --- | --- |
| OS file → RAM pane | Tauri `tauri://drag-enter` / `drag-over` / `drag-drop` events (delivers **paths**) | none — Tauri events |
| RAM card → disk pane | pointer-based in-app drag | **dnd-kit** |
| Disk card → RAM pane | pointer-based in-app drag | **dnd-kit** |
| Click to browse | native dialog | `tauri-plugin-dialog` |

See the drag-drop gotcha in [`02-architecture.md`](02-architecture.md): Tauri's native
drop handling suppresses HTML5 `drop` events, so `react-dropzone` will silently do
nothing while `dragDropEnabled` is `true`. We keep it enabled and drive drop-zone hover
styling from the Tauri events instead. dnd-kit is unaffected — it listens to pointer
events, not HTML5 DnD.

Drag affordances worth building:

- The disk pane highlights as a valid drop target only when a RAM file is being dragged
  **and it would fit** in remaining disk quota. A file that cannot fit should visibly
  refuse the drop rather than fail after release.
- While dragging, both meters show a **ghost segment** previewing the result: the RAM
  meter shows the space that would free, the disk meter the space that would fill. The
  consequence is visible before committing.
- Every drag has a button equivalent (`⇥` on each card). Drag-only is inaccessible and
  unusable on a projector.

## Visualization inventory

Forms chosen by the data's job, per `references/choosing-a-form.md`. Colors come from
the reference palette in `references/palette.md` — **run
`scripts/validate_palette.js` before shipping**, and re-run for dark mode against the
dark surface.

### 1. Store meters — segmented, per file

**Job:** part-to-whole against a fixed limit. **Form:** horizontal stacked bar drawn
inside a full-width track that represents the cap. Not a pie, not a donut — a pie cannot
show "how full is it", which is the entire question.

- One segment per file, **categorical** color, slots assigned in fixed order.
- The unused remainder is the track color (neutral), never a series color.
- 2px surface gap between adjacent segments; 4px rounded outer ends only.
- Direct-label segments wide enough to hold text; the rest resolve on hover.
- Cap marker at the right end with the limit value.

**Color assignment rule:** a file's color is bound to its `FileId` for the lifetime of
the session and does not change when other files are added or removed. Color follows the
entity, never its rank — if deleting one file repainted the others, the meter would look
like the data changed when nothing did. A file keeps its color when it moves from RAM to
disk, so you can watch the same colored block appear on the other side.

Past 7 files, fold the tail into a neutral gray **"Other"** segment. Never generate a
9th hue.

**Fill state** uses the status palette, and only for the *track*, never the segments:
under 70% neutral, 70–90% `warning`, over 90% `critical`. Status color always ships with
an icon and a label ("nearly full"), never color alone.

### 2. Live time series — two stacked charts, NOT one

**Job:** trend over time, during uploads and saves. Two measures: `ram_store_bytes`
(0–10 MB) and `process_rss_bytes` (~150–250 MB).

> [!warning]
> These two must **never** share one plot with two y-axes. A dual-axis chart is the
> single most common charting mistake — the two lines' crossings and relative heights
> would be artefacts of the two scales, and every conclusion drawn from them would be
> invented. Here it would be actively anti-pedagogical: it would imply the file bytes
> and the process footprint are comparable quantities, when the whole point is that one
> is a small part of the other.

Draw **two stacked area charts sharing one x-axis**, vertically aligned, each with its
own y-scale labelled in its own units:

- **Top:** RAM store bytes, y-axis fixed 0 → 10 MB so the cap is always the ceiling and
  the spike's size is honest.
- **Bottom:** process RSS, y-axis auto-scaled, annotated with a baseline reference line
  for "at rest" memory measured at startup.

Rolling window of 60 seconds at 4 Hz = 240 points. Keep the buffer in a ring in the
frontend store; never re-request history from Rust.

Annotate the moment of each upload/persist with a thin vertical rule and a small label,
so the causal link between the action and the spike is explicit rather than inferred.

### 3. Throughput ladder

**Job:** compare magnitude across a few labelled operations. **Form:** horizontal bar
chart, **sequential** single hue (this is magnitude, not identity — the operations are
not competing entities).

Extended to cover all four tiers, most recent measured value each: RAM write, RAM read,
disk write, disk read, database write, database read *(N/A — no read-back exists; omit
or gray out with a note)*, cloud write, cloud read *(same note)*. Since database and
cloud are one-way in this app, their "read" bars either do not appear or appear
explicitly marked "not available — see the tier graph", rather than silently missing.
Log scale is tempting because the gap is large; resist it for the RAM/disk pair
specifically — that gap *is* the lesson, and a log axis would hide exactly what we want
the learner to feel. Once cloud (network-bound) enters the picture the ladder spans
several more orders of magnitude; if the full linear ladder becomes unreadable, split
into two charts — "local" (RAM/disk/DB) and "network" (cloud) — rather than compressing
the local gap with a log axis that was working fine on its own.

Label the disk bars clearly when the artificial throttle is on. A fabricated number
presented as measured would poison the whole exercise.

### 3b. Streaming report chart

**Job:** a single comparison, shown once per streaming transfer, not a persistent
dashboard element. Full spec in [`07-streaming.md`](07-streaming.md). Two bars —
measured RSS delta during the stream vs. the buffered-equivalent (== file size, labelled
derived) — plus two stat tiles for the concurrency comparison (files-at-once, streaming
vs. buffered). Log scale is acceptable here specifically, since the goal of this one
chart is legibility of the ratio, not the visceral gap the tier-1 meters are for.

### 3c. Tier map

**Job:** show the four-tier graph itself — nodes and the one-directional edges between
them — as a standing reference in the Instruments drawer. **Form:** a small node-link
diagram, four boxes (RAM, Disk, Database, Cloud) sized by their cap on the same scale as
the meter tracks, arrows only in the valid directions (see
[`01-requirements.md`](01-requirements.md#moving-files-and-the-tier-graph)). Not
interactive; it exists so a learner can see at a glance that there is no arrow back to
RAM from anywhere. Use the ink/border tokens for boxes and arrows, not series colors —
this is structural chrome, not data.

### 4. File table

**Job:** more than a handful of rows, each carrying several attributes. **Form:** a
table, which is the honest answer and also the required accessible alternative to the
meters.

Columns: color swatch, name, size, % of its store's cap, location (a set — a file can be
in RAM+DB, Disk+Cloud, all four, etc. — render as small tier badges, not a single enum),
age. Sortable. `font-variant-numeric: tabular-nums` on the numeric columns.

### 5. Stat tiles (KPI row)

Four tiles in the primary row: RAM used, disk used, app memory, file counts. Value +
unit + a caption giving the cap. RAM used carries a sparkline of the last 60 s; the
others do not need one. Database and Cloud usage are **not** promoted to this row —
they are secondary tiers reached deliberately, and their meters live in their own
compact panels. These are stat tiles precisely because a one-bar bar chart would be
worse.

### Rejected on purpose

- **Donut/pie of file distribution.** Redundant with the meter, and it cannot express
  "how much room is left", which is the question the user actually has.
- **Treemap.** Needs many items to earn its complexity; we cap at a handful.
- **Gauge/speedometer.** Wastes space and reads less precisely than a linear meter.
- **Dual-axis combo chart.** See above.

## Accessibility and theming

- Legend present whenever ≥ 2 series are shown; ≤ 4 series are also direct-labelled, so
  identity is never carried by color alone.
- The file table is the table view required as a non-visual alternative to every chart.
- Dark mode uses the palette's **selected dark steps**, not an automatic inversion or a
  CSS filter. Validate the dark set against the dark surface separately.
- Three light-mode categorical slots sit below 3:1 contrast on the light surface — the
  relief rule applies, so those segments must carry visible direct labels or rely on the
  table view. Do not skip this.
- Offer the texture fill for the CVD / print / `forced-colors` case, off by default.
- Text (values, labels, legends) stays in ink tokens, never the series color; a colored
  swatch beside the text carries identity.
- Every chart has a hover layer: crosshair + tooltip on the time series, per-segment
  tooltip on the meters and bars.

## Motion

Motion is a teaching device here, so it gets a budget rather than being sprinkled.

- **Upload:** the new segment grows into the meter in real time from progress events.
- **Persist:** the card animates across the gap; its segment shrinks on the left and
  grows on the right in sync. This animation *is* the explanation of what persisting is.
- **Pull the plug:** the RAM pane goes dark and its segments collapse in one abrupt
  motion — deliberately harsher than every other transition in the app.
- Everything else: fast, subtle, and gated behind `prefers-reduced-motion`.

## Copy and tone

Labels teach. Prefer "volatile — cleared when the app closes" over "temporary". When a
quota is hit, say what happened, why, and what the user can do — that error state is a
designed screen, not a toast.

Consider a small "why?" affordance next to each instrument that opens the relevant note
from [`05-teaching-notes.md`](05-teaching-notes.md).

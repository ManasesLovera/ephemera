# 10 — Implementation status

Written 2026-08-04, same session as the build. This is the honest diff between the
spec (docs 00–09) and what actually got built under time pressure, so a future session
does not have to re-derive it by reading every file.

## What's real and tested

- **RAM tier**: full quota enforcement, path-based upload (no IPC byte copy), "pull the
  plug", per-file delete. Backed by `Arc<[u8]>` as specified.
- **Disk tier**: persist from RAM with real `fsync`, vault rescan (source-of-truth
  folder scanning), path-escape-safe filename sanitization, collision-safe naming.
- **Streaming tier**: `stream_upload_to_disk` copies in fixed 256 KB chunks, bypasses
  RAM entirely, produces a `StreamReport` with measured RSS baseline/peak/average and
  the buffered-vs-streaming concurrency comparison. Byte-exact-copy and concurrency-math
  are unit tested.
- **Database tier**: Postgres via `docker compose`, `BYTEA` storage, logical-vs-physical
  size distinction (`pg_total_relation_size`), 100 MB quota. Integration-tested against
  a real running container (also runs in CI via a Postgres service container).
- **Cloud tier**: GCS via hand-rolled REST + self-signed JWT service-account auth (no
  `google-cloud-storage` crate dependency — see the rationale in
  [`09-gcs-tier.md`](09-gcs-tier.md)). Integration-tested against the **real bucket**
  `gs://ephemera-vault-alterna` in project `alterna-489722` — upload, list, bytes-used,
  and delete all verified working end-to-end in this session.
- **Tier graph**: RAM→Disk, RAM→DB, RAM→Cloud, Disk→DB, Disk→Cloud all implemented via
  buttons on every file card. No edge leads back to RAM anywhere, matching the spec.
- **4 Hz metrics sampler**: walks the whole process tree via `sysinfo` (catches
  WebKitGTK's child processes, not just the Rust PID), feeds a live `metrics://tick`
  event the frontend charts off of.
- **25 automated tests**, all passing: quota edge cases, filename sanitization and path
  traversal rejection, vault rescan behavior (including hidden-file exclusion), stream
  byte-exact copy, concurrency-formula correctness, and live integration round-trips
  against both Postgres and GCS.
- **CI is green on a clean GitHub runner**: separate frontend (typecheck+build) and
  backend (fmt, clippy with `-D warnings`, build, test with a Postgres service
  container) jobs, plus a full Tauri Linux bundle build from scratch — all three passing
  as of the last push.

## Deliberate deviations from the original spec — and why

| Spec said | Built instead | Why |
| --- | --- | --- |
| Tailwind + shadcn/ui | Hand-written CSS using the same palette tokens | Faster to get a correct, working build under time pressure; visually equivalent, just no component library dependency risk |
| dnd-kit drag between RAM/disk panes | Buttons only (`→ Disk`, `→ DB`, `→ Cloud`) | The spec itself requires a button equivalent to every drag gesture for accessibility — this ships the required accessible path; the drag gesture is a follow-up, not a functional gap |
| Motion (Framer Motion) for card-crossing animation | No animation library yet | Same reasoning — functional correctness first |
| Full chart inventory (segmented meters with per-segment labels, tier map diagram, throughput ladder across all 4 tiers) | Simple stacked-bar meters + two sparklines (RAM store bytes, process RSS) in the Instruments drawer | The two-sparkline view already demonstrates the core "never share an axis" rule from `03-ui-and-visualization.md`; the fuller chart set is next |
| Spanish/English toggle (`docs/06-open-questions.md` item 4, answered "both") | English only | Not yet implemented — tracked below as the top open item |
| GCS `google-cloud-storage` crate | Direct REST + `jsonwebtoken`-signed service-account JWT | Avoids an unfamiliar, version-drift-prone crate; verified working against the real bucket in this session, which the original doc flagged as a risk to retire |

## Known gaps — pick these up next

1. **i18n (es/en toggle)** — was explicitly requested and answered "both" but not yet
   built. All UI strings currently live inline in JSX rather than a `strings.ts` module;
   extracting them is the first step.
2. **Segmented per-file meters with labels + hover, tier map diagram, full throughput
   ladder across all four tiers** — the richer chart set from `03-ui-and-visualization.md`
   beyond what's listed above as built.
3. **In-app drag between panes** (dnd-kit) and the crossing animation (Motion) — buttons
   work today; drag is the nicer interaction, not a blocker.
4. **Dark mode toggle UI** — the CSS variables for both themes exist and respond to
   `prefers-color-scheme`, but there's no in-app toggle stamping `data-theme` yet.
5. **Visual/screenshot verification** — the sandbox this was built in has no working
   screenshot mechanism (GNOME's D-Bus screenshot portal denied access, no `grim`, and
   `sudo apt install` requires an interactive password this session doesn't have). The
   app was verified by running `cargo tauri dev` successfully (process alive, expected
   ~200 MB RSS matching the documented WebKit-overhead teaching point, no runtime
   panics) and by the CI Tauri bundle build succeeding — but nobody has actually looked
   at the rendered window. **Do this first** in the next session with a display attached.
6. **`upload_to_ram` currently takes a single `path: String`.** Multi-file batch
   validation (spec: "validate the whole batch against remaining quota as a whole") is
   not implemented — each file is validated independently as the frontend loops over a
   multi-select.

## Live GCP resources created this session

For the next session's awareness — these are real, billable-but-free-tier resources
that now exist:

- Project: `alterna-489722` (Cloud Storage API enabled here)
- Bucket: `gs://ephemera-vault-alterna` (region `us-central1`, uniform bucket-level access)
- Service account: `ephemera-app@alterna-489722.iam.gserviceaccount.com`, granted
  `roles/storage.objectAdmin` **on that bucket only**
- A key for it exists locally at `src-tauri/gcs-key.json` (gitignored, never pushed)

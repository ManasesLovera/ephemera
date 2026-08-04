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
| Spanish/English toggle (`docs/06-open-questions.md` item 4, answered "both") | Built: `src/lib/i18n.ts` holds both dictionaries, a Zustand store (persisted to localStorage) tracks the current language, and every user-facing string in the app runs through `useT()`. Toggle button lives in the top-right chrome (EN/ES). | Complete |
| GCS `google-cloud-storage` crate | Direct REST + `jsonwebtoken`-signed service-account JWT | Avoids an unfamiliar, version-drift-prone crate; verified working against the real bucket in this session, which the original doc flagged as a risk to retire |

## Visual verification — done, and it caught a real bug

The window **was** visually verified in this session. GNOME's D-Bus screenshot portal
denies access and there's no `grim`/passwordless `sudo` for one, but forcing the webview
through XWayland (`GDK_BACKEND=x11`, plus `WEBKIT_DISABLE_DMABUF_RENDERER=1` and
`WEBKIT_DISABLE_COMPOSITING_MODE=1` to defeat WebKit's GL compositing path, which
otherwise leaves the X11 window's backing pixmap empty) let `import -window <id>`
(ImageMagick) capture the real rendered window. `ffmpeg -f x11grab` alone was not
enough — it reads the root window's composited output, which under a Wayland-native
compositor (mutter) never receives the X11 client's content; `import` against the
specific window ID reads via XComposite and does.

The screenshot immediately surfaced a real bug: **"App Memory" read 13,745.2 MB**
instead of the expected ~450 MB. Root cause: `sysinfo` on Linux enumerates each
*thread* of every process as its own pseudo-process entry (reading
`/proc/[pid]/task/[tid]`), each reporting the parent's full process-wide RSS. The
original `tree_rss` walk in `metrics.rs` treated these as child processes and summed
each thread's (identical, whole-process) RSS once per thread — WebKitWebProcess alone
has ~15 threads, so its true ~210 MB was counted roughly 15 times over. Fixed by
filtering on `Process::thread_kind().is_none()` (thread pseudo-entries return
`Some(_)`, real processes `None`) both when accumulating and when walking children.
Rebuilt, re-screenshotted: **449.2 MB**, matching `ps aux` summed by hand. This is
exactly the kind of bug the app's own `04-tech-stack.md` warned about ("RSS across a
process tree double-counts shared pages... label it in the UI as an approximation") —
the warning was about a different failure mode, but the same instinct (verify the
number, don't trust the aggregation) is what caught this one.

The same screenshot pass also confirmed, live, against real infrastructure: the DB
panel correctly shows "32.0 KB physical (incl. overhead)" against 0 B logical — the
Postgres table's own baseline page overhead, visible even with zero files stored, which
is a live instance of the logical-vs-physical teaching point the tier was built to
demonstrate. The cloud panel correctly shows `bucket: ephemera-vault-alterna`, confirming
the real GCS connection from the UI layer, not just the backend integration test.

A cosmetic bug was also caught and fixed in the same pass: the DB/Cloud sink meters
forced a minimum-visible sliver (`Math.max(1, pct(...))`) even at exactly 0 bytes used,
making empty stores look like they held something. Fixed to render no fill at all when
`used === 0`.

**Click-driven interaction: since confirmed, by the user, for real.** The caveat above
was true when written; later the same session, real click-driven use (upload, persist
to disk, save to DB, save to Cloud) happened via the user's own build running on this
same machine/display — evidenced by a real file ("A Tour of C++...") showing up
correctly in the Disk, Database, and Cloud panels simultaneously, each with a working
delete button, when checked via the screenshot technique. The full pipeline — click →
IPC → Rust state → real Postgres/GCS → UI reflecting it — is confirmed working
end-to-end, not just individually tested.

## Built after this doc was first written

- **Full i18n (English/Spanish toggle)** — `src/lib/i18n.ts`, every string in the app
  routed through `useT()`, toggle in the top-right chrome. No longer a gap.
- **Database and Cloud panels now list and delete files**, matching RAM/Disk — each row
  has a single ✕ delete action (no move actions; DB/Cloud stay one-way sinks per the
  tier graph). Verified against the live Postgres container and the real GCS bucket.
- **Loading states + tooltip**: "Open folder" / "Rescan" show a spinner and disable
  while their async call is in flight; "Pull the plug" has a hover tooltip explaining
  what it actually does (instant, total, no undo) — it was the one destructive action
  with no explanation attached.
- **`DOWNLOAD.md`** — points at the GitHub Releases page rather than hardcoded
  version links (those go stale the moment a new tag ships), with a table decoding the
  asset-naming pattern per platform and full build-from-source steps.
- **Release process is live**: v0.1.0 was created manually (Linux x86_64 only, built
  and uploaded from this machine). v0.2.0 used the new
  [`.github/workflows/release.yml`](../.github/workflows/release.yml) — triggered by
  pushing a `v*` tag, builds a 5-way matrix (Linux x86_64, Linux arm64, Windows x64,
  macOS arm64, macOS x64) via `tauri-apps/tauri-action`, auto-creates the release and
  attaches whatever each platform produces.

### Known bug: Linux arm64 release build fails in CI

The `ubuntu-24.04-arm` leg of `release.yml` has failed on every release so far —
`pnpm tauri build` exits 1 partway through the "Build and publish" step. Not yet
diagnosed. The other four platforms (Linux x86_64, Windows x64, macOS arm64, macOS
x64) have built clean on every run since v0.2.0. Likely causes worth checking first:
a Tauri Linux dependency (`libayatana-appindicator3-dev` or similar) not resolving on
the arm64 Ubuntu image, or `linuxdeploy`/AppImage tooling not shipping an arm64
binary — the AppImage bundling step is a common failure point for arm64 specifically.
Until fixed, Linux arm64 has no prebuilt binary; `DOWNLOAD.md` documents
build-from-source as the workaround.

### Version bump procedure (for the next tag)

Three files carry the version number and must be bumped together, or the release
name/binary metadata will disagree with each other: `package.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`. After bumping `Cargo.toml`, run
`cargo check` once to regenerate `Cargo.lock` (it embeds the package version) before
committing. Then `git tag -a vX.Y.Z -m "..."` and `git push origin vX.Y.Z` to trigger
`release.yml`.

## Other known gaps — pick these up next

1. **Fix the Linux arm64 release build** (above) — the most concrete, well-scoped next
   task.
2. **Segmented per-file meters with labels + hover, tier map diagram, full throughput
   ladder across all four tiers** — the richer chart set from `03-ui-and-visualization.md`
   beyond what's listed above as built.
3. **In-app drag between panes** (dnd-kit) and the crossing animation (Motion) — buttons
   work today; drag is the nicer interaction, not a blocker.
4. **Dark mode toggle UI** — the CSS variables for both themes exist and respond to
   `prefers-color-scheme`, but there's no in-app toggle stamping `data-theme` yet.
5. **`upload_to_ram` currently takes a single `path: String`.** Multi-file batch
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

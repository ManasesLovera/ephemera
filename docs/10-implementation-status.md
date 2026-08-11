# 10 — Implementation status

Originally written 2026-08-04 for the Tauri + React build. Updated 2026-08-11 after the
Tauri → Slint migration (see [`migration/PLAN.md`](../migration/PLAN.md) for how it was
run, and [`migration/tasks.json`](../migration/tasks.json) for the full per-phase
record). This is the honest diff between the spec (docs 00–09) and what's actually
built, so a future session doesn't have to re-derive it by reading every file.

## What's real and tested

- **RAM tier**: full quota enforcement, direct-read upload via a native file picker (no
  serialization boundary to cross anymore), "pull the plug", per-file delete. Backed by
  `Arc<[u8]>` as specified.
- **Disk tier**: persist from RAM with real `fsync`, vault rescan (source-of-truth
  folder scanning), path-escape-safe filename sanitization, collision-safe naming.
- **Streaming tier**: `stream_upload_to_disk` copies in fixed 256 KB chunks, bypasses
  RAM entirely, produces a `StreamReport` with measured RSS baseline/peak/average and
  the buffered-vs-streaming concurrency comparison, now surfaced in a Slint modal
  (Phase 5 of the migration — the React build measured this but never displayed it).
  Byte-exact-copy and concurrency-math are unit tested.
- **Database tier**: Postgres via `docker compose`, `BYTEA` storage, logical-vs-physical
  size distinction (`pg_total_relation_size`), 100 MB quota. Integration-tested against
  a real running container (also runs in CI via a Postgres service container).
- **Cloud tier**: GCS via hand-rolled REST + self-signed JWT service-account auth.
  Integration-tested against the real bucket `gs://ephemera-vault-alterna` in project
  `alterna-489722`.
- **Tier graph**: RAM→Disk, RAM→DB, RAM→Cloud, Disk→DB, Disk→Cloud all implemented via
  buttons on every file card. No edge leads back to RAM anywhere, matching the spec.
- **4 Hz metrics sampler**: walks the whole process tree via `sysinfo`, feeding
  `ShellState::push_metrics` which mutates a persistent Slint `VecModel` in place
  (fixed during Phase 4's review — an earlier version rebuilt the model from scratch
  every tick, churning the heap this app exists to measure honestly).
- **In-app drag-and-drop between panes** (Phase 5): dragging a file card between
  RAM/Disk/DB/Cloud, via Slint's `DragArea`/`DropArea`. OS-level drag-and-drop of files
  from *outside* the window (e.g. a file manager) is **not** implemented — Slint
  1.17's `DataTransfer` API carries no file paths at all, only images/text/same-process
  payloads. This is a framework limitation, not an oversight; see
  [`02-architecture.md`](02-architecture.md#getting-file-bytes-into-the-store).
- **CI is green on a clean GitHub runner**: `Core` (fmt, clippy `-D warnings`, test with
  a Postgres service container) and `Slint App` (fmt, clippy, test, release build) jobs
  both pass — confirmed on a real PR run
  ([actions/runs/31502637812](https://github.com/ManasesLovera/ephemera/actions/runs/31502637812)),
  not just locally.

## The Tauri → Slint migration

Run via an automated multi-agent pipeline (`migration/dispatch.sh` + `pr.sh`): each
phase generated code in an isolated git worktree, ran `cargo test` as a deterministic
gate, got two independent LLM code reviews, then squash-merged on double approval. See
[`migration/PLAN.md`](../migration/PLAN.md) for the full design and the honesty
tradeoffs it accepted (no human read of generated code — the test gate and review
verdicts were the only checks).

**Real problems the pipeline hit and how they were resolved** (kept here rather than
just in `migration/tasks.json` notes, since they're the kind of thing worth knowing
before trusting an unattended agent pipeline again):

- **`opencode`'s `--dir` flag pointed at the main checkout, not the task's worktree.**
  Every "isolated" parallel generate task was actually writing into the same directory,
  racing `git checkout` against each other. Root-caused by reading the agent's own
  transcript (`git status` showed `On branch main` when it should have shown the task's
  branch) — fixed by passing `--dir "$PWD"` instead of a hardcoded path.
- **Worktrees living inside the repo tree escaped into an unrelated workspace-root
  `Cargo.toml`** (from a separate, untracked test-infrastructure effort also present in
  this repo). Cargo's manifest search walks the filesystem, not git boundaries, so
  every worktree's `cargo test` picked up that outer workspace file and failed. Fixed
  by relocating worktrees outside the repo's directory subtree entirely.
- **A no-op generation silently reported "done".** One task's agent spent its whole
  turn budget researching a real blocker (Slint's `Path` element rejecting `for` loops)
  and never wrote a file; the dispatch script's test gate trivially passed against
  unchanged code. Fixed by failing the task outright when no commit was produced.
- **Two `opencode-go/*` models silently exited with no error right before their first
  file write**, across two separate tasks and two different models — never diagnosed
  further; switching to `google/gemini-3.6-flash` worked immediately both times.
- **A review model hallucinated a missing CI step that was actually present**,
  reproducibly, across two different diffs. Resolved not by retrying indefinitely but
  by waiting for the real GitHub Actions run on the PR — stronger ground truth than
  either LLM review — which passed cleanly, confirming the hallucination.
- **Parallel branches extending the same shared files** (`app.slint`, `main.rs`,
  `model.rs`) from a common base produced real git merge conflicts and a genuine
  component name collision (two different `FileCard` components) — resolved by hand,
  serially, rebasing each branch onto the previous one's merge.

## Deliberate deviations from the original spec — and why

| Spec said | Built instead | Why |
| --- | --- | --- |
| Real OS drag-and-drop of files into the RAM pane | Click-to-browse file picker only; in-app drag between panes works | Slint 1.17's `DataTransfer` API has no file-path support at all — a framework limitation confirmed by reading the crate source, not a port that was skipped |
| Vault path persists between runs (`01-requirements.md` line 182, a documented MUST) | In-memory only, resets to the default path every launch | **Pre-existing gap, not introduced by the Slint migration** — `ephemera_core::set_vault_path` was always in-memory-only, even in the original Tauri build. The migration ported the existing (already non-compliant) behavior faithfully rather than silently adding new behavior the Tauri build never had. Worth fixing, but as its own task with its own review, not folded into a UI-framework migration |
| Full chart inventory (segmented meters with per-segment labels, tier map diagram, throughput ladder across all 4 tiers) | Simple stacked-bar meters + two bar-sparklines (RAM store bytes, process RSS) in the Instruments drawer | Same scope decision as the original Tauri build — the two-sparkline view demonstrates the core "never share an axis" rule; the fuller chart set remains a follow-up |
| Spanish/English toggle | Not carried over to the Slint UI | The React build's i18n (`src/lib/i18n.ts` + Zustand-persisted toggle) was removed along with `src/`. Re-implementing i18n in Slint (which has its own translation mechanism, `@tr()`) is unstarted — a real gap worth flagging, not silently dropped |
| GCS `google-cloud-storage` crate | Direct REST + `jsonwebtoken`-signed service-account JWT | Unchanged from the original Tauri build; carried over as-is |

## Manual verification of the Slint app

The release binary (`cargo build --release` in `crates/ephemera-app`) was launched and
confirmed to stay alive with no errors or panics in its log output, on this machine
(GNOME/Wayland). **No screenshot tool was available in this environment** (no `grim`,
`slurp`, `gnome-screenshot`, `scrot`; ImageMagick's `import` requires X11, which this
Wayland-only session doesn't have) to also verify pixel-level layout against the old
Tauri screenshots — so visual/pixel parity with the Tauri build is **not confirmed**,
only that the app launches, runs its 4 Hz sampler, and doesn't crash. This is an honest
gap, not a claim of full visual verification; if you can run a Wayland screenshot tool
in this environment, that's the natural next check.

Structural/functional parity was verified through: the automated code reviews at every
migration phase (each diff checked against `docs/03-ui-and-visualization.md`'s layout
spec and `CLAUDE.md`'s rules), the full `cargo test` suite across both crates, and the
RSS validation below.

## Memory validation

Full-app RSS was measured against a fresh build of the pre-migration Tauri app under
the same idle/workload conditions —
[`migration/outputs/P6-T1/REPORT.md`](../migration/outputs/P6-T1/REPORT.md) has the
full methodology and raw numbers. Headline result: **123 MiB vs. 393 MiB tree RSS at
idle (3.2x less memory)**, confirming the migration's premise — the WebKitGTK child
processes alone (~222 MB) exceeded the entire Slint app's footprint.

### Version bump procedure (for the next tag)

Two files carry the version number and must be bumped together:
`crates/ephemera-app/Cargo.toml` and `crates/ephemera-core/Cargo.toml`. After bumping,
run `cargo check` in each crate once to regenerate its `Cargo.lock` before committing.
Then `git tag -a vX.Y.Z -m "..."` and `git push origin vX.Y.Z` to trigger
`release.yml`. **Note:** `release.yml` was rewritten for Slint binaries during this
migration (Phase 7) but has not yet been exercised by an actual tag push — the CI
workflow's build steps were verified on a real PR run, but the release workflow's
matrix build (5 platforms) has not been. Treat the first post-migration release as a
first real test of that workflow, not a known-working path.

## Other known gaps — pick these up next

1. **Vault path persistence** (pre-existing gap, see table above) — the most
   concretely scoped next task, and a documented MUST requirement.
2. **Re-implement i18n** in Slint (`@tr()` or a similar mechanism) — dropped when
   `src/` was removed, not yet rebuilt.
3. **Verify `release.yml`'s matrix build** with an actual tag push (see above).
4. **Segmented per-file meters with labels + hover, tier map diagram, full throughput
   ladder across all four tiers** — the richer chart set from
   `03-ui-and-visualization.md` beyond what's listed above as built.
5. **Dark mode** — not implemented in the Slint UI; the original CSS-variable-based
   approach doesn't carry over directly and needs a Slint-native design.
6. **Pixel-level visual verification** against the old Tauri screenshots, once a
   Wayland screenshot tool is available in this environment (see above).
7. **`upload_to_ram` currently takes a single path.** Multi-file batch validation
   (spec: "validate the whole batch against remaining quota as a whole") is not
   implemented — unchanged from the original Tauri build.

## Live GCP resources

Still real, billable-but-free-tier resources from the original build session, unchanged
by the migration:

- Project: `alterna-489722` (Cloud Storage API enabled here)
- Bucket: `gs://ephemera-vault-alterna` (region `us-central1`, uniform bucket-level access)
- Service account: `ephemera-app@alterna-489722.iam.gserviceaccount.com`, granted
  `roles/storage.objectAdmin` **on that bucket only**
- A key for it exists locally at `crates/ephemera-app/gcs-key.json` (gitignored, never
  pushed) — moved from `src-tauri/gcs-key.json` during the migration.

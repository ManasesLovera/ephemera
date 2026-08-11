# Tauri → Slint migration plan

> [!summary]
> Multi-phase plan to replace the Tauri/React shell with a native Slint UI, keeping
> the Rust tier logic (RAM/disk/Postgres/GCS) intact. Execution is delegated to two
> local CLIs — `agy` and `opencode` — running in the background against a shared
> file-based task tracker (`tasks.json`) so I (the orchestrating Claude session) can
> dispatch work, poll status, and reconcile review feedback without re-deriving
> context each time.

## Task State and Progress Notes

Tasks use the state flow `todo → running → done|failed`. The old `pending` value is
accepted as a legacy alias for `todo`, but new or reset tasks must use `todo`.
Every task carries a `notes` object with three lists: `done`, `missing`, and `todo`.
Agents and the orchestrator must update these notes whenever work starts, reaches a
meaningful milestone, encounters a gap, or completes verification. The `todo` list
must always state the remaining actionable work, including an empty list when nothing
remains.

## Autonomy level (confirmed with the user)

This pipeline is fully automated end to end, with **no human read of the generated
code and no human click to merge**. Concretely, per generation branch:

1. `agy`/`opencode` generate the code in an isolated git worktree.
2. `dispatch.sh` deterministically runs `cargo test` in that worktree — not
   self-reported by the model, actually executed.
3. Both review models (`deepseek-v4-pro` and `gemini-3.6-flash-high`) review the
   diff and each must end their output with a literal `VERDICT: PASS` line.
4. A `*-MERGE` task (no LLM call, pure `gh`/`git`) checks: tests passed AND both
   reviews are `done` AND both logs contain `VERDICT: PASS`. Only if all three hold
   does it open a PR and immediately squash-merge it into `main`.

I'm not reading the generated code myself, per your instruction — the test run and
the two independent review verdicts are the only gate. That means the safety of
this pipeline rests entirely on: the review prompts actually catching real bugs,
`cargo test` coverage being meaningful, and neither review model rubber-stamping.
None of those are guaranteed. Concretely: **a real, tagged, publicly-downloadable
app can get a broken `main` from this pipeline with no human step in between.**
Watch `./migration/status.sh` and the PR list on GitHub while phases run — that's
your practical way to catch a bad merge shortly after it happens, since nothing here
pauses for you beforehand.

## Why this shape

- **Goal stated by the user:** cut Tauri's RAM baseline (WebKitGTK) by moving to a
  native-render toolkit (Slint), without spending this session's tokens re-generating
  every file by hand.
- **Constraint:** must reuse `agy` and `opencode` (already installed, already
  authenticated — confirmed via `opencode providers list`: Google, Nvidia, OpenCode Go).
- **Constraint:** code generation uses `gemini-3.6` (via `agy`) and `deepseek-v4-flash`
  (via `opencode-go`), alternated per task to spread load. Code review uses
  `deepseek-v4-pro` (via `opencode-go`) and `gemini-3.6-flash-high` (via `agy`), run
  **both** against every generated task for cross-validation — a rewrite this size
  benefits from two independent reviewers more than generation benefits from two
  independent authors.
- **Safety constraint (mine, not requested — flagging it):** background CLI agents
  editing the same working tree concurrently will stomp on each other. Every
  generation task therefore runs in its own `git worktree` on a throwaway branch
  (`migration/<task-id>`); nothing touches `main` until a human (you) merges a
  reviewed branch. Review tasks run read-only against that branch's diff.

## Model routing

| Role | CLI | Model | Invocation |
| --- | --- | --- | --- |
| Generate A | `agy` | `gemini-3.6-flash-medium` | `agy --print --model gemini-3.6-flash-medium` |
| Generate B | `opencode` | `opencode-go/deepseek-v4-flash` | `opencode run --model opencode-go/deepseek-v4-flash` |
| Review A | `opencode` | `opencode-go/deepseek-v4-pro` | `opencode run --model opencode-go/deepseek-v4-pro` |
| Review B | `agy` | `gemini-3.6-flash-high` | `agy --print --model gemini-3.6-flash-high --effort high` |

`gemini-3.6` has no `-pro` variant in `agy models` today, only `-flash-{high,medium,low}`
— using `-medium` for generation (cheaper) and `-high` for review, per the user's
"high" instruction, is the closest literal match. Flag this if a pro tier lands later.

## Phases

### Phase 0 — Inventory & feasibility spike (research only, no writes)

Confirms scope before committing to the rewrite. Cheap, safe to run unattended.

- **P0-T1** (generate/research): Enumerate every Tauri IPC command
  (`src-tauri/src/commands/{ram,disk,stream,db,cloud,config}.rs` — 6 modules) and
  every frontend component (`src/components/*.tsx` — 8 files) with their props/events,
  producing a migration inventory table.
- **P0-T2** (generate): Spike a single Slint window that renders the RAM tier's
  `Meter`/`StatTile` at 4 Hz from a Rust timer, measuring RSS before/after against the
  current Tauri build — this is the number that validates the whole migration.
- **P0-T3/T4** (review A/B): Review the spike's measurement methodology — is the RSS
  comparison apples-to-apples (same workload, same build profile)?

**Gate:** if the spike doesn't show a meaningful RSS drop, stop here — re-evaluate
egui/iced instead of continuing (see prior conversation turn).

### Phase 1 — Decouple backend from Tauri

- **P1-T1** (generate A): Extract the 6 command modules' business logic into a
  `#[tauri::command]`-free `ephemera-core` crate/module — plain functions returning
  plain Rust types, no `tauri::State`/`Window` coupling.
- **P1-T2/T3** (review A/B): Verify no behavior changed (fsync-on-every-write, 10/20/100 MB
  caps, one-way RAM→disk/db/gcs invariant from `CLAUDE.md` all still hold).

### Phase 2 — Slint shell + state bindings

- **P2-T1** (generate B): Slint window shell + property model mirroring `src/store`,
  wired to `ephemera-core` via direct Rust calls/callbacks (no IPC serialization).
- **P2-T2/T3** (review A/B): Check the property update path doesn't introduce a second
  buffering copy of file bytes in the UI layer (the "frontend never holds authoritative
  bytes" rule from `CLAUDE.md` applies just as much to Slint's Rust side as it did to
  the webview).

### Phase 3 — Port tier panels

- **P3-T1** (generate A): `RamPane` → Slint.
- **P3-T2** (generate B): `DiskPane` → Slint.
- **P3-T3** (generate A): `SinkPanel` (DB + GCS) → Slint.
- **P3-T4/T5** (review A/B, once per generated component): correctness + the "labelled
  as simulated" rule for the disk latency throttle.

### Phase 4 — Dashboard & charts

- **P4-T1** (generate B): Port `Instruments`/`Meter`/`StatTile` 4 Hz dashboard.
  Slint has no chart library — this is hand-rolled `Path`/canvas widgets, the highest-risk
  phase for effort blowup.
- **P4-T2** (generate A): Enforce the "no dual-axis charts" rule — RAM store bytes and
  RSS as two stacked panels sharing an x-axis.
- **P4-T3/T4** (review A/B): Visual/data correctness — no fabricated numbers, simulated
  values still labelled.

### Phase 5 — Drag & drop + file cards

- **P5-T1** (generate B): `FileCard` + drag-and-drop using Slint's pointer-event
  primitives (no native web DnD equivalent — this is a from-scratch interaction model).
- **P5-T2** (generate A): `StreamModal` (RAM-bypass streaming path + completion report).
- **P5-T3/T4** (review A/B).

### Phase 6 — Validation against the spec

- **P6-T1** (generate A or human): Re-run the RSS/measurement comparison from P0-T2
  against the full app, not just the spike.
- **P6-T2/T3** (review A/B): Full pass against `docs/01-requirements.md` MUST/SHOULD
  list and the `CLAUDE.md` rules (real metaphor, no temp files, fsync, no fabricated
  numbers).

### Phase 7 — CI cutover

- **CI-T1** (generate A): update `.github/workflows/ci.yml`/`release.yml` to
  build/test the Slint binary instead of the Tauri/pnpm/vite pipeline.
  `release.yml`'s `prerelease: false` must not change — that's a hard rule from
  `CLAUDE.md` and the review prompt treats any change to it as a blocking failure.
- **CI-T2/T3** (review A/B) + **CI-T1-MERGE**: same double-review + test + auto-merge
  gate as every other phase.
- Packaging (Slint's platform bundling differs from Tauri's bundler) and the actual
  version-tag release are **not** part of this automated pipeline — `CLAUDE.md`
  requires tags/releases only on your explicit trigger, and that line hasn't moved.

## Orchestration mechanics

See `migration/tasks.json` (the shared tracker), `migration/prompts/*.md` (one prompt
per task), and `migration/dispatch.sh` / `migration/status.sh`.

- `./migration/dispatch.sh <task-id>` launches one task: creates its git worktree (for
  `generate` tasks), runs the assigned CLI with the assigned model against the task's
  prompt file, writes stdout to `migration/logs/<task-id>.log`, and updates
  `tasks.json`'s status field (`pending → running → done|failed`) before it exits.
- Run `dispatch.sh` itself via a **backgrounded** Bash call so the harness notifies me
  on completion — no polling loop needed.
- `./migration/status.sh` prints the current state of every task from `tasks.json` —
  this is the "tracker" other agents and I both read, so nothing needs a live message
  bus, just a shared file and file-locking via `jq`'s atomic rewrite-and-move.
- Review tasks (`kind: review`) run read-only against the generation branch's diff —
  their prompt includes `git diff main...migration/<gen-task-id>` output, not repo
  write access.

## Known risks I'm flagging, not hiding

- `dispatch.sh` passes `--dangerously-skip-permissions` to `agy` and `--auto` to
  `opencode` so unattended background runs don't hang waiting on an interactive
  approval prompt. Those processes can execute shell commands and edit files
  without a human in the loop. Mitigation: each generation task is confined to its
  own disposable git worktree/branch.
- **Auto-merge to `main` is live** (see "Autonomy level" above) — `gh pr merge
  --squash --delete-branch` runs unattended against the real
  `ManasesLovera/ephemera` GitHub repo, which has tagged releases people can
  download. The only gates are automated (`cargo test` + two LLM review verdicts).
  This was an explicit choice you made when asked; it is the single most
  consequential switch in this whole scaffold.
- `opencode-go/deepseek-v4-flash` has had intermittent API reliability issues.
  `dispatch.sh` retries automatically through `opencode-go/kimi-k2.7-code` then
  `opencode-go/glm-5.2` on failure (see `fallback_models` in `tasks.json`), and
  records which model actually produced the code in `model_used` — check that field
  if a PR's authorship looks surprising.

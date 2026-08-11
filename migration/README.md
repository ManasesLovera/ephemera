# Migration orchestration scaffold

Read `PLAN.md` first — this file is just how to operate the tracker.

**The tracker moved from `tasks.json` to sqlite (`tasks.db`) on 2026-08-11.**
It's no longer just the migration-pipeline's tracker either — it now also
holds ordinary project tasks (known gaps, follow-up work), each tagged with a
`target_version`. If you're a background agent looking for what to work on
next, read [`docs/11-background-agents-guide.md`](../docs/11-background-agents-guide.md)
first — it's written specifically for that handoff. `tasks.json` is kept only
as the frozen historical record of the completed Tauri→Slint migration; it is
not updated anymore and nothing reads it at runtime.

## Layout

- `tasks.db` — the shared task tracker (sqlite). Schema in `schema.sql`.
  `status` moves `todo → running → done|failed`. Every task can have
  `done`/`missing`/`todo` notes (table `task_notes`) and a `context` field
  explaining what it is and which files/docs are relevant — read `context`
  before starting a task. Use `./tasks.sh` to read/write it; don't hand-edit
  the db. `tools/json_to_sqlite.py` is the one-time script that seeded it
  from the old `tasks.json`, kept for provenance.
- `tasks.sh` — the general task manager CLI: `list`, `show <id>`, `pending`,
  `ready <id>`, `add`, `set-status`, `note`, `set-field`. Run
  `./tasks.sh --help` for the full reference.
- `prompts/*.md` — one prompt per pipeline task, referenced by a task row's
  `prompt_file` column.
- `dispatch.sh <task-id>` — runs one **pipeline** task (`kind` = generate,
  review, or merge — the ones with `cli`/`model`/`worktree` set): checks its
  dependencies are `done`, creates a git worktree for `generate` tasks,
  invokes the assigned CLI/model, logs to `logs/<task-id>.log`, updates
  `tasks.db` on start and finish. Ordinary `bug`/`feature`/`chore` tasks
  (the known-gaps list) are performed directly by whoever picks them up, not
  through `dispatch.sh` — see the background-agents guide linked above.
- `status.sh` — prints the tracker as a table (thin wrapper over
  `./tasks.sh list`).
- `logs/`, `outputs/`, `worktrees/` — gitignored, all runtime state.

Concurrent tasks are supported. Each task writes its own log and output, while
`dispatch.sh` serializes `tasks.db` read/modify/move updates with a process lock so
parallel reviews cannot overwrite one another's status, model, or progress notes.

## Progress Notes

Keep each task's `notes` field current throughout execution. It must always explain:

- `done`: work completed and verification performed
- `missing`: known gaps, blockers, or failed verification
- `todo`: the remaining actionable checklist

Update the notes when a task starts, after meaningful progress, when verification
changes, and when the task finishes. Do not leave a task with only a status change;
the notes are the durable progress report and must include the remaining todo list.

## Running a task

Run `dispatch.sh` itself as a **backgrounded** shell call (not `&` inside the
script) so the calling agent gets a completion notification instead of having to
poll:

```bash
./migration/dispatch.sh P0-T1   # run in background from the harness, not inline
```

Check progress any time with:

```bash
./migration/status.sh
```

The status table lists status and title. Run `./tasks.sh show <id>` for the full
`context` and `done`/`missing`/`todo` notes when the complete progress report is needed.

or read a specific task's log directly:

```bash
tail -f migration/logs/P0-T2.log
```

## Ordering

Respect `depends_on` in `tasks.db` — `dispatch.sh` refuses to start a task whose
dependencies aren't `done` yet. Within a phase, independent tasks (e.g. `P3-T1`,
`P3-T2`, `P3-T3`) can be dispatched in parallel since each runs in its own git
worktree/branch and can't collide on disk.

## Merging (automatic)

Every `generate` task has a matching `<task-id>-MERGE` task depending on it and on
its review tasks. Dispatching a `-MERGE` task runs `pr.sh`, which:

1. Opens a PR for the branch (`pr.sh open <gen-task-id>`).
2. Checks the generation task's `tests_status == "passed"` and that every
   dependent review task is `done` with a `VERDICT: PASS` line in its log
   (`pr.sh merge <gen-task-id>`).
3. Only if both hold: `gh pr merge --squash --delete-branch` — this lands on the
   real `main` of `ManasesLovera/ephemera`, a repo with tagged public releases.

No human clicks merge in this pipeline (confirmed with the user). If any gate
fails, `pr.sh merge` exits non-zero, the `-MERGE` task is marked `failed` in
`tasks.db`, and the PR is left open for a human to look at.

You can inspect a branch's diff and gate state any time without waiting for the
merge task:

```bash
git diff main...migration/P1-T1
./tasks.sh show P1-T1   # includes tests_status, model_used
tail migration/logs/P1-T2.log migration/logs/P1-T3.log   # look for VERDICT: lines
```

## Before you run this unattended

`dispatch.sh` passes `--dangerously-skip-permissions` (agy) / `--auto` (opencode)
so background runs don't block on an interactive approval prompt, and `-MERGE`
tasks push real branches and merge real PRs with `gh`. Read the "Known risks"
section of `PLAN.md` — this is not a sandboxed dry run.

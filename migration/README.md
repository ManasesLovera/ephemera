# Migration orchestration scaffold

Read `PLAN.md` first — this file is just how to operate the tracker.

## Layout

- `tasks.json` — the shared task tracker. `status` moves
  `todo → running → done|failed`. `pending` is accepted as a legacy alias for
  `todo`. Every task has a `notes` object with `done`, `missing`, and `todo` lists.
  Anything reading this file (me, you, another agent) sees the current state without
  needing a live connection to whatever is running.
- `prompts/*.md` — one prompt per task, referenced by `tasks.json`'s `prompt_file`.
- `dispatch.sh <task-id>` — runs one task: checks its dependencies are `done`,
  creates a git worktree for `generate` tasks, invokes the assigned CLI/model, logs
  to `logs/<task-id>.log`, updates `tasks.json` on start and finish.
- `status.sh` — prints the tracker as a table.
- `logs/`, `outputs/`, `worktrees/` — gitignored, all runtime state.

Concurrent tasks are supported. Each task writes its own log and output, while
`dispatch.sh` serializes `tasks.json` read/modify/move updates with a process lock so
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

The status table includes the first remaining todo item. Read the full `notes` object
from `tasks.json` when the complete progress report is needed.

or read a specific task's log directly:

```bash
tail -f migration/logs/P0-T2.log
```

## Ordering

Respect `depends_on` in `tasks.json` — `dispatch.sh` refuses to start a task whose
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
`tasks.json`, and the PR is left open for a human to look at.

You can inspect a branch's diff and gate state any time without waiting for the
merge task:

```bash
git diff main...migration/P1-T1
jq '.tasks[] | select(.id=="P1-T1") | {tests_status, model_used}' tasks.json
tail migration/logs/P1-T2.log migration/logs/P1-T3.log   # look for VERDICT: lines
```

## Before you run this unattended

`dispatch.sh` passes `--dangerously-skip-permissions` (agy) / `--auto` (opencode)
so background runs don't block on an interactive approval prompt, and `-MERGE`
tasks push real branches and merge real PRs with `gh`. Read the "Known risks"
section of `PLAN.md` — this is not a sandboxed dry run.

# 11 — Guide for background agents

Written 2026-08-11, when the task tracker moved from `migration/tasks.json` to
`migration/tasks.db` (sqlite). Read this if you were prompted with something
like "list known pending tasks" or "perform task GAP-VAULT-PERSIST" with
little other context — it tells you what this project is, where the task
tracker lives, and how to use it without re-deriving any of this.

## What this project is, in one paragraph

Ephemera is a native Slint desktop app (`crates/ephemera-app`, over
`crates/ephemera-core`) that teaches the storage hierarchy — RAM, disk,
database, cloud — by actually moving real file bytes between a real in-memory
buffer, real files with real `fsync`, a real Postgres `BYTEA` column, and a
real GCS bucket, one-way, never back to RAM. It's a working, shipped app
(v1.0.0, tagged) with green CI. **Read `/CLAUDE.md` before touching anything**
— it has the git/release workflow rules, the project's hard invariants (never
fabricate a measured number, `fsync` on every disk write, no dual-axis
charts, etc.), and the doc reading order. `docs/10-implementation-status.md`
is the up-to-date honest diff between spec and reality; it's where the known
gaps below come from.

## The task tracker

`migration/tasks.db` is a sqlite database. It holds two kinds of rows:

1. **Historical migration-pipeline tasks** (`P0-T1` … `P9-Tn`, all
   `target_version = '1.0.0'`) — the record of the automated multi-agent
   pipeline that ported this app from Tauri+React to Slint. All `done`. You
   don't need to act on these; they're provenance, described in
   `migration/PLAN.md` if you're curious how that pipeline worked.
2. **Ordinary engineering tasks** (`GAP-*`, `TASK-*`, and whatever you or a
   future agent adds), each tagged with a `target_version` — currently
   `1.1.0` holds the 7 known gaps from `docs/10-implementation-status.md`
   plus the task that built this tracker. These are the tasks a prompt like
   "list known pending tasks" is asking about.

The schema is in `migration/schema.sql`. Full field reference:

| column | meaning |
| --- | --- |
| `id` | short slug, primary key |
| `title` | one line |
| `kind` | `bug` \| `feature` \| `chore` \| `research` \| `generate` \| `review` \| `merge` — the last four are pipeline-specific |
| `status` | `todo` \| `running` \| `done` \| `failed` \| `blocked` |
| `target_version` | the release this ships in, e.g. `"1.1.0"` |
| `context` | freeform: what this is, which files/docs are relevant, why it exists — **read this before starting a task**, it's written for exactly this handoff |
| `phase`, `cli`, `model`, `worktree`, `branch`, `tests_status` | pipeline-only fields, null for ordinary tasks |

Related tables: `task_depends` (task_id, depends_on), `task_fallback_models`
(pipeline-only), `task_notes` (task_id, kind ∈ {done,missing,todo}, position,
text).

## `migration/tasks.sh` — the tool to use

Don't query the sqlite file by hand unless `tasks.sh` genuinely can't do
what you need; it exists so both humans and agents have one consistent
interface. Run `./migration/tasks.sh --help` for the full command list. The
ones you'll use most:

```bash
./migration/tasks.sh pending                    # "list known pending tasks"
./migration/tasks.sh list --version 1.1.0        # everything targeting a release
./migration/tasks.sh show GAP-VAULT-PERSIST      # full context + notes for one task
./migration/tasks.sh set-status GAP-VAULT-PERSIST running   # claim it
./migration/tasks.sh note GAP-VAULT-PERSIST done "added ~/.config/ephemera/vault-path.toml, read at startup"
./migration/tasks.sh set-status GAP-VAULT-PERSIST done       # finish it
```

## The workflow this is designed for

When told to **"perform task X"**:

1. `./migration/tasks.sh show X` — read `context` and any existing notes in
   full before writing code. It names the relevant docs and files; read
   those too (`docs/00`–`docs/10`, and the specific crate/file it points at).
2. `./migration/tasks.sh set-status X running` as soon as you start, so a
   second agent asked to "list pending tasks" doesn't duplicate the work.
3. Do the work, following `/CLAUDE.md`'s rules (the metaphor must stay real,
   no fabricated numbers, `fsync` on every disk write, etc.) and this
   project's normal test/lint gates (`cargo fmt`, `cargo clippy -D warnings`,
   `cargo test`).
4. `./migration/tasks.sh note X done "<what you verified, not just what you wrote>"`
   — and a `missing` note for anything you couldn't finish or verify. Be as
   honest here as `docs/10-implementation-status.md` is; that file's whole
   value is that its gaps are true.
5. `./migration/tasks.sh set-status X done` (or `failed`, with a `missing`
   note explaining why, if you had to stop).
6. Per `/CLAUDE.md`: **do not commit or push on your own initiative.** Leave
   the working tree for the user to review unless they explicitly asked you
   to commit/push in the same turn.

When told to **"list known pending tasks"**: run `./migration/tasks.sh
pending`, don't guess from memory — statuses change as work lands.

## When told to work a task "as a background task": worktree → PR → CI

This is a distinct, heavier workflow from the "perform task X" one above. Use
it only when the user explicitly frames the request this way — phrases like
"work this as a background task", "do this in a worktree", "open a PR for
this" — not for an ordinary "perform task X" or "fix this" said in the main
checkout. Assume you may currently be sitting in the repo root, not already
inside a worktree; you're responsible for creating one.

**Worktrees live outside the repo tree, always** — never under `.worktrees/`
or any other path inside this checkout. This repo carries an untracked
workspace-root `Cargo.toml` and `src-tauri/` left over from a separate,
untracked test-infrastructure effort (`git status` shows them as `??`).
Cargo's manifest search walks the filesystem, not git worktree boundaries, so
any worktree created *inside* this tree inherits that stray root manifest and
`cargo test` breaks for reasons that look unrelated to your change — this
exact failure mode is documented in
[`docs/10-implementation-status.md`](10-implementation-status.md) from when
the migration pipeline hit it. `migration/dispatch.sh` fixed it once already
by relocating worktrees outside the repo; do the same:

```bash
worktree=~/dev/worktrees/ephemera/<branch>
git worktree add -b <branch> "$worktree" main   # run from the repo root
cd "$worktree"
```

**Branch naming** follows Conventional Commits types, same as this project's
commit messages: `<type>/<slug>`, e.g. `feat/vault-persist`,
`fix/disk-fsync-race`, `chore/tasks-schema-bump`. Derive `<type>` from the
task's `kind` (`bug` → `fix`, `feature` → `feat`, `chore` → `chore`,
`research` → `docs`) and `<slug>` from the task id or a short kebab-case
description.

Procedure, once you're in the worktree:

1. Implement the task per its `context` (see above) and every invariant in
   `/CLAUDE.md` — the metaphor must stay real, `fsync` on every disk write,
   no dual-axis charts, etc.
2. Self-review before opening the PR: `cargo fmt`, `cargo clippy -D
   warnings`, `cargo test`, and a real read-through of your own diff (the
   `code-review` skill is appropriate here). Fix what you find — don't open a
   PR you haven't reviewed.
3. Commit with a Conventional Commits message (see `/CLAUDE.md`'s Git
   section for the exact format rules).
4. Push the branch and open the PR yourself: `git push -u origin <branch>`,
   then `gh pr create` with a real title (Conventional Commits style) and a
   description (Summary + Test plan, same shape as the PR-creation
   instructions in the top-level system prompt).
5. Monitor CI: `gh pr checks <branch> --watch` (or `gh run watch` on the run
   it triggers). If a check fails, pull the failure log (`gh run view <id>
   --log-failed`), fix the root cause in the worktree, commit, push, and
   watch again. Repeat with no iteration cap until CI is green — don't stop
   after one attempt and report back short of that, unless the failure turns
   out to require a decision outside the task's stated scope (in which case
   stop and ask, same as any other ambiguous blocker).
6. Leave the PR open and unmerged for the user — merging is a shared-state,
   hard-to-reverse action outside this workflow's scope even after CI is
   green. Report the PR URL and the worktree path when you're done; don't
   remove the worktree yourself.

**This is a scoped exception to `/CLAUDE.md`'s "commit/push only on explicit
request, each turn" rule** — it applies only inside this specific,
explicitly-invoked workflow. It does not license committing or pushing for
any other request, including plain "perform task X" done in the main
checkout, which still follows the no-auto-commit rule above.

## What NOT to do

- Don't hand-edit `migration/tasks.db` or `migration/schema.sql` outside
  `tasks.sh` unless you're changing the schema itself (and if you do, update
  `migration/tools/json_to_sqlite.py` and this doc to match).
- Don't resurrect `migration/tasks.json` as a second source of truth — it's
  kept only as the historical record `json_to_sqlite.py` was built from.
- Don't run `migration/dispatch.sh` / `migration/pr.sh` expecting them to
  pick up new `GAP-*`/`TASK-*` work automatically — those two scripts are
  the old LLM-dispatch pipeline (spawns a CLI agent in a git worktree, opens
  a PR, auto-merges on double review). They still work and still read/write
  `tasks.db`, but they're wired for `kind IN (generate, review, merge)`
  tasks with `cli`/`model`/`worktree` set, not for the plain engineering
  tasks this guide is about. Perform those tasks directly instead.

#!/usr/bin/env bash
# Dispatch one migration task from tasks.json to its assigned CLI/model.
# Meant to be invoked itself as a backgrounded process by the orchestrating
# Claude session, so completion shows up as a task notification rather than
# needing a poll loop here.
#
# kind="generate" tasks: run in an isolated git worktree/branch, gated by an
#   automatic cargo test run, then a PR is opened for the branch.
# kind="review"   tasks: read-only, diff a branch, must end their log with an
#   explicit VERDICT: PASS|FAIL line (enforced by prompts/*review*.md).
# kind="merge"    tasks: no LLM call at all — deterministically checks the
#   target generation task's test gate + every dependent review's VERDICT,
#   and only then calls pr.sh to squash-merge into main. This is the one
#   irreversible step in the pipeline; see PLAN.md's risk note.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"
TASKS="$ROOT/tasks.json"
# Review tasks may run concurrently. Keep tracker read/modify/move operations
# serialized without adding lock state to the repository.
TASKS_LOCK="${TMPDIR:-/tmp}/ephemera-migration-tasks.lock"

TASK_ID="${1:?usage: dispatch.sh <task-id>}"

task=$(jq -c --arg id "$TASK_ID" '.tasks[] | select(type=="object" and .id==$id)' "$TASKS")
if [ -z "$task" ]; then
  echo "unknown task id: $TASK_ID" >&2
  exit 1
fi

cli=$(jq -r '.cli // empty' <<<"$task")
model=$(jq -r '.model // empty' <<<"$task")
effort=$(jq -r '.effort // empty' <<<"$task")
kind=$(jq -r '.kind' <<<"$task")
prompt_rel=$(jq -r '.prompt_file // empty' <<<"$task")
worktree_rel=$(jq -r '.worktree // empty' <<<"$task")
branch=$(jq -r '.branch // empty' <<<"$task")
target_task=$(jq -r '.target_task // empty' <<<"$task")

mapfile -t fallback_models < <(jq -r '.fallback_models // [] | .[]' <<<"$task")
candidate_models=("$model" "${fallback_models[@]}")

# Every dependency must be "done" before we run. `pending` remains accepted for
# trackers created before the `todo` state was introduced.
unmet=$(jq -r --arg id "$TASK_ID" '
  ((.tasks[] | select(type=="object" and .id==$id) | .depends_on) // []) as $deps
  | [ .tasks[] | select(type=="object" and (.id as $t | $deps | index($t)) != null and .status != "done") | .id ]
  | join(",")
' "$TASKS")
if [ -n "$unmet" ]; then
  echo "task $TASK_ID has unmet dependencies: $unmet" >&2
  exit 1
fi

mark_status() {
  local status="$1" ts_field="$2"
  (
    flock -x 9
    local tmp
    tmp=$(mktemp)
    jq --arg id "$TASK_ID" --arg st "$status" --arg tf "$ts_field" --arg ts "$(date -u +%FT%TZ)" '
      .updated_at = $ts
      | (.tasks[] | select(type=="object" and .id==$id) | .status) = $st
      | (.tasks[] | select(type=="object" and .id==$id) | .[$tf]) = $ts
    ' "$TASKS" > "$tmp"
    mv "$tmp" "$TASKS"
  ) 9>"$TASKS_LOCK"
}

mark_field() {
  local field="$1" value="$2"
  (
    flock -x 9
    local tmp
    tmp=$(mktemp)
    jq --arg id "$TASK_ID" --arg f "$field" --arg v "$value" '
      (.tasks[] | select(type=="object" and .id==$id) | .[$f]) = $v
    ' "$TASKS" > "$tmp"
    mv "$tmp" "$TASKS"
  ) 9>"$TASKS_LOCK"
}

mark_note() {
  local section="$1" value="$2"
  (
    flock -x 9
    local tmp
    tmp=$(mktemp)
    jq --arg id "$TASK_ID" --arg section "$section" --arg value "$value" '
      (.tasks[] | select(type=="object" and .id==$id) | .notes) //= {done: [], missing: [], todo: []}
      | (.tasks[] | select(type=="object" and .id==$id) | .notes[$section]) += [$value]
    ' "$TASKS" > "$tmp"
    mv "$tmp" "$TASKS"
  ) 9>"$TASKS_LOCK"
}

mkdir -p "$ROOT/logs" "$ROOT/outputs/$TASK_ID"
LOG="$ROOT/logs/$TASK_ID.log"

mark_status "running" "started_at"
mark_note "done" "Task dispatched and execution started."

# --- merge tasks: no LLM, deterministic gate + gh, then exit early ---------
if [ "$kind" = "merge" ]; then
  [ -n "$target_task" ] || { echo "merge task $TASK_ID missing target_task" >&2; mark_status "failed" "finished_at"; exit 1; }
  status="done"
  {
    echo "=== opening PR for $target_task ==="
    bash "$ROOT/pr.sh" open "$target_task"
    echo "=== checking merge gate for $target_task ==="
    bash "$ROOT/pr.sh" merge "$target_task"
  } >"$LOG" 2>&1 || status="failed"
  cp "$LOG" "$ROOT/outputs/$TASK_ID/output.txt" 2>/dev/null || true
  mark_status "$status" "finished_at"
  echo "task $TASK_ID -> $status (log: $LOG)"
  exit 0
fi

prompt_file="$ROOT/$prompt_rel"
if [ ! -f "$prompt_file" ]; then
  echo "missing prompt file: $prompt_file" >&2
  mark_status "failed" "finished_at"
  exit 1
fi

runcli() {
  local run_model="$1"
  local prompt
  prompt="$(cat "$prompt_file")"
  case "$cli" in
    agy)
      # NOTE: `--print` greedily consumes whatever token comes immediately
      # after it on the command line, even another flag like `--model` —
      # confirmed twice now (once via `--prompt "$prompt"`, once via a
      # naive stdin-redirect fix that still put `--print` before `--model`):
      # both times agy replied as if its prompt literally was "--model".
      # The actual fix is ordering: put `--print` LAST, with nothing after
      # it but the stdin redirect, so there's no token for it to eat.
      local effort_args=()
      [ -n "$effort" ] && effort_args=(--effort "$effort")
      agy --model "$run_model" \
        "${effort_args[@]}" \
        --sandbox \
        --dangerously-skip-permissions \
        --add-dir "$REPO" \
        --print \
        < "$prompt_file"
      ;;
    opencode)
      local variant_args=()
      [ -n "$effort" ] && variant_args=(--variant "$effort")
      # --dir must be the CALLER'S cwd ($PWD, set by run_with_fallback's
      # `cd "$cwd"` before invoking runcli), NOT a hardcoded $REPO. opencode
      # treats --dir as its actual working directory regardless of process
      # cwd — hardcoding $REPO here made every opencode-driven generate task
      # operate directly on the main checkout instead of its isolated
      # worktree, causing concurrent tasks to race `git checkout` against
      # each other in the same directory. Confirmed via reflog + agent logs
      # during Phase 3 (P3-T1/P3-T2 both wrote into $REPO on Aug 11 2026).
      opencode run \
        --model "$run_model" \
        "${variant_args[@]}" \
        --auto \
        --dir "$PWD" \
        "$prompt"
      ;;
    *)
      echo "unknown cli: $cli" >&2
      return 1
      ;;
  esac
}

# Try the primary model, then each fallback in order, in the given cwd. Stops
# at the first success; logs every failed attempt before moving on so a bad
# run is still auditable.
run_with_fallback() {
  local cwd="$1"
  local m
  for m in "${candidate_models[@]}"; do
    echo "--- attempting model: $m ---" >>"$LOG"
    if ( cd "$cwd" && runcli "$m" ) >>"$LOG" 2>&1; then
      mark_field "model_used" "$m"
      return 0
    fi
    echo "--- model failed: $m ---" >>"$LOG"
  done
  return 1
}

status="done"
if [ "$kind" = "generate" ] && [ -n "$worktree_rel" ]; then
  # Worktrees live OUTSIDE the repo tree, not under migration/worktrees/.
  # Reason: a separate untracked effort (.agents/, root Cargo.toml) staged a
  # workspace-root Cargo.toml at $REPO — since worktrees are physical
  # subdirectories on disk, cargo's upward manifest search ignores git
  # worktree boundaries and escapes into that outer workspace file, breaking
  # `cargo test` for every generated crate. Keeping worktrees off the repo's
  # directory subtree entirely avoids that collision regardless of what the
  # repo root's Cargo.toml declares.
  WORKTREE_BASE="${EPHEMERA_MIGRATION_WORKTREE_BASE:-$HOME/dev/ephemera-migration-worktrees}"
  mkdir -p "$WORKTREE_BASE"
  worktree="$WORKTREE_BASE/$TASK_ID"
  if [ ! -d "$worktree" ]; then
    git -C "$REPO" worktree add -b "$branch" "$worktree" main >>"$LOG" 2>&1 \
      || git -C "$REPO" worktree add "$worktree" "$branch" >>"$LOG" 2>&1
  fi
  if run_with_fallback "$worktree"; then
    # Agents routinely `cargo build`/`cargo test` inside new crates whose
    # target/ dir isn't yet covered by .gitignore, then `git add -A` below
    # would stage the whole build output (thousands of files, breaks pushes
    # with 408s). Belt-and-suspenders: ensure every target/ dir under the
    # worktree is ignored before staging, regardless of what the agent wrote.
    if ! grep -qxF '**/target/' "$worktree/.gitignore" 2>/dev/null; then
      echo '**/target/' >> "$worktree/.gitignore"
    fi
    # Deterministic test gate: don't trust the agent's self-reported
    # cargo check/test output in its own response, run it ourselves.
    echo "--- running cargo test in $worktree ---" >>"$LOG"
    if [ -f "$worktree/Cargo.toml" ] || [ -f "$worktree/src-tauri/Cargo.toml" ]; then
      cargo_dir="$worktree"
      [ -f "$worktree/Cargo.toml" ] || cargo_dir="$worktree/src-tauri"
      if ( cd "$cargo_dir" && cargo test --quiet ) >>"$LOG" 2>&1; then
        mark_field "tests_status" "passed"
      else
        mark_field "tests_status" "failed"
        status="failed"
      fi
    else
      echo "no Cargo.toml found — skipping cargo test, marking tests_status=skipped" >>"$LOG"
      mark_field "tests_status" "skipped"
    fi
    git -C "$worktree" add -A >>"$LOG" 2>&1 || true
    git -C "$worktree" commit -m "feat(migration): $(jq -r '.title' <<<"$task") ($TASK_ID)" >>"$LOG" 2>&1 || true
  else
    status="failed"
    mark_field "tests_status" "not run"
  fi
else
  # research/review tasks run read-only from the repo root (or the branch
  # under review, if one is set) — no worktree, no write scope expected.
  if [ -n "$branch" ]; then
    echo "--- reviewing branch: $branch ---" >>"$LOG"
    git -C "$REPO" --no-pager diff "main...$branch" >>"$LOG" 2>&1 || true
  fi
  run_with_fallback "$REPO" || status="failed"
fi

cp "$LOG" "$ROOT/outputs/$TASK_ID/output.txt" 2>/dev/null || true
mark_status "$status" "finished_at"
if [ "$status" = "done" ]; then
  mark_note "done" "Task execution finished; see the task output and log for verification."
else
  mark_note "missing" "Task execution failed; inspect the task log before retrying."
fi

echo "task $TASK_ID -> $status (log: $LOG)"

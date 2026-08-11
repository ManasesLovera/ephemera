#!/usr/bin/env bash
# The task manager tool: a small sqlite-backed CLI over migration/tasks.db.
#
# Built for two audiences: a human running `./migration/tasks.sh list`, and a
# background agent that was told "list known pending tasks" and needs to find
# and act on them without any other context. See docs/11-background-agents-guide.md
# for the full workflow this is designed around.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB="$ROOT/tasks.db"
LOCK="${TMPDIR:-/tmp}/ephemera-tasks.lock"

sqerr() { echo "error: $*" >&2; exit 1; }

# Single-quote-escape a value for interpolation into a SQL literal.
esc() { printf '%s' "${1//\'/\'\'}"; }

now() { date -u +%FT%TZ; }

sql() {
  ( flock -x 9; sqlite3 "$DB" "$@" ) 9>"$LOCK"
}

usage() {
  cat <<'EOF'
usage: tasks.sh <command> [args]

  list [--status S] [--version V] [--phase N] [--kind K]
                                  Table of tasks, optionally filtered.
  pending                         Shortcut for: list --status todo
  show <id>                       Full detail for one task: fields, deps,
                                   fallback models, and done/missing/todo notes.
  ready <id>                      Exit 0 and print nothing if every dependency
                                   of <id> is status=done; otherwise print the
                                   unmet dependency ids and exit 1.
  add --id ID --title T --kind K [--status S] [--target-version V]
      [--phase N] [--context TEXT] [--depends-on ID[,ID...]]
                                  Create a new task. status defaults to todo.
  set-status <id> <status>        todo|running|done|failed|blocked. Stamps
                                   started_at on -> running, finished_at on
                                   -> done|failed.
  note <id> <done|missing|todo> <text>
                                  Append one note line (kept, not replaced).
  set-field <id> <field> <value>  Escape hatch for any other column
                                   (cli, model, model_used, effort, branch,
                                   worktree, tests_status, context, ...).

Every command reads/writes migration/tasks.db directly with sqlite3, guarded
by a flock so concurrent agents don't race each other's updates.
EOF
}

cmd_list() {
  local where="1=1"
  while [ $# -gt 0 ]; do
    case "$1" in
      --status) where="$where AND status='$(esc "$2")'"; shift 2 ;;
      --version) where="$where AND target_version='$(esc "$2")'"; shift 2 ;;
      --phase) where="$where AND phase=$(esc "$2")"; shift 2 ;;
      --kind) where="$where AND kind='$(esc "$2")'"; shift 2 ;;
      *) sqerr "list: unknown flag $1" ;;
    esac
  done
  {
    echo -e "ID\tVERSION\tKIND\tSTATUS\tTITLE"
    sql -separator $'\t' "
      SELECT id, COALESCE(target_version,'-'), kind, status, title
      FROM tasks WHERE $where
      ORDER BY (target_version IS NULL), target_version, (phase IS NULL), phase, id;
    "
  } | column -t -s $'\t'
}

cmd_show() {
  local id="${1:?usage: tasks.sh show <id>}"
  local json
  json=$(sql -json "SELECT * FROM tasks WHERE id='$(esc "$id")';" | jq '.[0] // empty')
  [ -n "$json" ] || sqerr "unknown task id: $id"

  jq -r '
    "id:             " + .id,
    "title:          " + .title,
    "kind:           " + .kind,
    "status:         " + .status,
    "target_version: " + (.target_version // "null"),
    "phase:          " + ((.phase // "null") | tostring)
  ' <<<"$json"
  [ "$(jq -r '.cli // empty' <<<"$json")" != "" ] && \
    jq -r '"cli/model:      " + .cli + " / " + (.model_used // .model) + " (effort: " + (.effort // "default") + ")"' <<<"$json"
  [ "$(jq -r '.tests_status // empty' <<<"$json")" != "" ] && \
    jq -r '"tests_status:   " + .tests_status' <<<"$json"
  jq -r '
    "started_at:     " + (.started_at // "null"),
    "finished_at:    " + (.finished_at // "null")
  ' <<<"$json"
  local ctx
  ctx=$(jq -r '.context // empty' <<<"$json")
  if [ -n "$ctx" ]; then
    echo
    echo "context:"
    echo "$ctx" | sed 's/^/  /'
  fi

  local deps
  deps=$(sql -separator ', ' "SELECT d.depends_on || ' [' || t.status || ']' FROM task_depends d JOIN tasks t ON t.id=d.depends_on WHERE d.task_id='$(esc "$id")';")
  [ -n "$deps" ] && { echo; echo "depends_on: $deps"; }

  for kind in done missing todo; do
    local notes
    notes=$(sql "SELECT '  - ' || text FROM task_notes WHERE task_id='$(esc "$id")' AND kind='$kind' ORDER BY position;")
    if [ -n "$notes" ]; then
      echo
      echo "notes.$kind:"
      echo "$notes"
    fi
  done
}

cmd_ready() {
  local id="${1:?usage: tasks.sh ready <id>}"
  local unmet
  unmet=$(sql -separator ',' "
    SELECT d.depends_on FROM task_depends d
    JOIN tasks t ON t.id = d.depends_on
    WHERE d.task_id='$(esc "$id")' AND t.status != 'done';
  ")
  if [ -n "$unmet" ]; then
    echo "$unmet"
    return 1
  fi
  return 0
}

cmd_add() {
  local id="" title="" kind="" status="todo" version="" phase="" context="" deps=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --id) id="$2"; shift 2 ;;
      --title) title="$2"; shift 2 ;;
      --kind) kind="$2"; shift 2 ;;
      --status) status="$2"; shift 2 ;;
      --target-version) version="$2"; shift 2 ;;
      --phase) phase="$2"; shift 2 ;;
      --context) context="$2"; shift 2 ;;
      --depends-on) deps="$2"; shift 2 ;;
      *) sqerr "add: unknown flag $1" ;;
    esac
  done
  [ -n "$id" ] && [ -n "$title" ] && [ -n "$kind" ] || sqerr "add: --id, --title, and --kind are required"

  local phase_sql="NULL"
  [ -n "$phase" ] && phase_sql="$(esc "$phase")"

  sql "
    INSERT INTO tasks (id, title, kind, status, target_version, phase, context, created_at, updated_at)
    VALUES ('$(esc "$id")', '$(esc "$title")', '$(esc "$kind")', '$(esc "$status")',
            $( [ -n "$version" ] && echo "'$(esc "$version")'" || echo NULL ),
            $phase_sql,
            $( [ -n "$context" ] && echo "'$(esc "$context")'" || echo NULL ),
            '$(now)', '$(now)');
  "
  if [ -n "$deps" ]; then
    IFS=',' read -ra dep_arr <<<"$deps"
    for d in "${dep_arr[@]}"; do
      sql "INSERT OR IGNORE INTO task_depends (task_id, depends_on) VALUES ('$(esc "$id")', '$(esc "$d")');"
    done
  fi
  echo "added $id"
}

cmd_set_status() {
  local id="${1:?usage: tasks.sh set-status <id> <status>}"
  local status="${2:?usage: tasks.sh set-status <id> <status>}"
  case "$status" in
    todo|running|done|failed|blocked) ;;
    *) sqerr "set-status: invalid status '$status' (expected todo|running|done|failed|blocked)" ;;
  esac
  local ts_field=""
  [ "$status" = "running" ] && ts_field="started_at"
  [ "$status" = "done" ] || [ "$status" = "failed" ] && ts_field="finished_at"

  local extra=""
  [ -n "$ts_field" ] && extra=", $ts_field = '$(now)'"
  sql "UPDATE tasks SET status='$(esc "$status")', updated_at='$(now)'$extra WHERE id='$(esc "$id")';"
  echo "$id -> $status"
}

cmd_note() {
  local id="${1:?usage: tasks.sh note <id> <done|missing|todo> <text>}"
  local kind="${2:?usage: tasks.sh note <id> <done|missing|todo> <text>}"
  local text="${3:?usage: tasks.sh note <id> <done|missing|todo> <text>}"
  case "$kind" in done|missing|todo) ;; *) sqerr "note: kind must be done|missing|todo" ;; esac
  local next_pos
  next_pos=$(sql "SELECT COALESCE(MAX(position)+1, 0) FROM task_notes WHERE task_id='$(esc "$id")' AND kind='$(esc "$kind")';")
  sql "INSERT INTO task_notes (task_id, kind, position, text) VALUES ('$(esc "$id")', '$(esc "$kind")', $next_pos, '$(esc "$text")');"
  sql "UPDATE tasks SET updated_at='$(now)' WHERE id='$(esc "$id")';"
}

cmd_set_field() {
  local id="${1:?usage: tasks.sh set-field <id> <field> <value>}"
  local field="${2:?usage: tasks.sh set-field <id> <field> <value>}"
  local value="${3:?usage: tasks.sh set-field <id> <field> <value>}"
  case "$field" in
    cli|model|model_used|effort|branch|worktree|tests_status|context|prompt_file|target_task) ;;
    *) sqerr "set-field: unsupported field '$field'" ;;
  esac
  sql "UPDATE tasks SET \"$field\"='$(esc "$value")', updated_at='$(now)' WHERE id='$(esc "$id")';"
}

[ -f "$DB" ] || sqerr "no tasks.db at $DB — run migration/tools/json_to_sqlite.py once, or migration/schema.sql to bootstrap an empty one"

cmd="${1:-}"; shift || true
case "$cmd" in
  list) cmd_list "$@" ;;
  pending) cmd_list --status todo ;;
  show) cmd_show "$@" ;;
  ready) cmd_ready "$@" ;;
  add) cmd_add "$@" ;;
  set-status) cmd_set_status "$@" ;;
  note) cmd_note "$@" ;;
  set-field) cmd_set_field "$@" ;;
  ""|-h|--help|help) usage ;;
  *) sqerr "unknown command: $cmd (run with --help)" ;;
esac

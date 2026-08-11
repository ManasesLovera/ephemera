#!/usr/bin/env bash
# Drive the entire remaining task graph to completion, unattended.
# Repeatedly scans tasks.json, dispatches any task whose deps are all "done",
# retries failed tasks up to MAX_ATTEMPTS, and stops when either everything
# is done or no further progress is possible in a full pass.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TASKS="$ROOT/tasks.json"
MAX_ATTEMPTS=2
declare -A attempts

log() { echo "[$(date -u +%FT%TZ)] $*"; }

while true; do
  mapfile -t ids < <(jq -r '.tasks[].id' "$TASKS")
  all_done=true
  progressed=false

  for id in "${ids[@]}"; do
    status=$(jq -r --arg id "$id" '.tasks[] | select(.id==$id) | .status' "$TASKS")
    [ "$status" = "done" ] && continue
    all_done=false
    [ "$status" = "running" ] && continue

    unmet=$(jq -r --arg id "$id" '
      ((.tasks[] | select(.id==$id) | .depends_on) // []) as $deps
      | [ .tasks[] | select((.id as $t | $deps | index($t)) != null and .status != "done") | .id ]
      | length
    ' "$TASKS")
    [ "$unmet" != "0" ] && continue

    a="${attempts[$id]:-0}"
    if [ "$status" = "failed" ] && [ "$a" -ge "$MAX_ATTEMPTS" ]; then
      continue
    fi

    attempts[$id]=$((a + 1))
    log "dispatching $id (attempt $((a + 1)))"
    bash "$ROOT/dispatch.sh" "$id"
    progressed=true
  done

  if $all_done; then
    log "ALL TASKS DONE"
    break
  fi
  if ! $progressed; then
    log "STUCK — no dispatchable task this pass:"
    jq -r '.tasks[] | select(.status!="done") | "  \(.id): \(.status) (attempts=\(env.ATTEMPTS // "?"))"' "$TASKS"
    exit 1
  fi
done

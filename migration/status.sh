#!/usr/bin/env bash
# Quick status table for the migration task tracker.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
jq -r '
  ["ID","PHASE","KIND","STATUS","CLI/MODEL","TITLE","NEXT TODO"],
  (.tasks[] | [.id, (.phase|tostring), .kind, .status, (.cli+"/"+.model), .title, ((.notes.todo // ["notes missing"]) | first)])
  | @tsv
' "$ROOT/tasks.json" | column -t -s $'\t'

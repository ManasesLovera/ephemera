#!/usr/bin/env bash
# Quick status table for the task tracker. Thin wrapper over tasks.sh, kept
# as its own entry point since it predates tasks.sh and other tooling/muscle
# memory still calls it directly.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$ROOT/tasks.sh" list "$@"

#!/usr/bin/env python3
"""One-time import of migration/tasks.json into migration/tasks.db.

Kept for provenance (how the sqlite tracker was seeded) and as a reference
if tasks.json is ever needed again. Not part of normal operation once
tasks.db exists — migration/tasks.sh is the tool for day-to-day use.

Every task in tasks.json belonged to the Tauri -> Slint migration that
shipped as v1.0.0, so all imported rows get target_version = "1.0.0".
"""
import json
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
JSON_PATH = ROOT / "tasks.json"
DB_PATH = ROOT / "tasks.db"
SCHEMA_PATH = ROOT / "schema.sql"
IMPORTED_TARGET_VERSION = "1.0.0"


def normalize_status(status: str) -> str:
    return "todo" if status == "pending" else status


def main() -> None:
    data = json.loads(JSON_PATH.read_text())
    tasks = [t for t in data["tasks"] if isinstance(t, dict)]

    conn = sqlite3.connect(DB_PATH)
    conn.executescript(SCHEMA_PATH.read_text())
    cur = conn.cursor()

    for t in tasks:
        cur.execute(
            """
            INSERT INTO tasks (
                id, phase, title, kind, status, target_version,
                cli, model, model_used, effort, prompt_file, worktree,
                branch, target_task, tests_status, started_at, finished_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO NOTHING
            """,
            (
                t["id"],
                t.get("phase"),
                t["title"],
                t["kind"],
                normalize_status(t.get("status", "todo")),
                IMPORTED_TARGET_VERSION,
                t.get("cli"),
                t.get("model"),
                t.get("model_used"),
                t.get("effort"),
                t.get("prompt_file"),
                t.get("worktree"),
                t.get("branch"),
                t.get("target_task"),
                t.get("tests_status"),
                t.get("started_at"),
                t.get("finished_at"),
            ),
        )

        for dep in t.get("depends_on", []) or []:
            cur.execute(
                "INSERT OR IGNORE INTO task_depends (task_id, depends_on) VALUES (?, ?)",
                (t["id"], dep),
            )

        for i, model in enumerate(t.get("fallback_models", []) or []):
            cur.execute(
                "INSERT OR IGNORE INTO task_fallback_models (task_id, position, model) VALUES (?, ?, ?)",
                (t["id"], i, model),
            )

        notes = t.get("notes") or {}
        for kind in ("done", "missing", "todo"):
            for i, text in enumerate(notes.get(kind, []) or []):
                cur.execute(
                    "INSERT OR IGNORE INTO task_notes (task_id, kind, position, text) VALUES (?, ?, ?, ?)",
                    (t["id"], kind, i, text),
                )

    conn.commit()
    count = cur.execute("SELECT COUNT(*) FROM tasks").fetchone()[0]
    conn.close()
    print(f"imported {len(tasks)} tasks from tasks.json -> {count} rows now in {DB_PATH}")


if __name__ == "__main__":
    sys.exit(main())

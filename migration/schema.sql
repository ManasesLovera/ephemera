-- Ephemera task tracker schema (sqlite).
--
-- Supersedes migration/tasks.json, which was flat JSON tailored to the
-- Tauri -> Slint migration's LLM-dispatch pipeline (see PLAN.md). This
-- schema keeps every field that pipeline needs (cli/model/worktree/branch/
-- tests_status) but normalizes depends_on, fallback_models, and the
-- done/missing/todo notes into real tables, and adds target_version +
-- context so the tracker also works for ordinary engineering tasks that
-- don't go through dispatch.sh at all (e.g. the post-1.0 known-gap list).
--
-- kind: research | generate | review | merge | bug | feature | chore
-- status: todo | running | done | failed | blocked ("pending" from the old
--   JSON tracker is normalized to "todo" on import)

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS tasks (
  id             TEXT PRIMARY KEY,
  phase          INTEGER,                 -- migration-pipeline phase number, NULL for non-pipeline tasks
  title          TEXT NOT NULL,
  kind           TEXT NOT NULL,
  status         TEXT NOT NULL DEFAULT 'todo',
  target_version TEXT,                    -- e.g. "1.0.0", "1.1.0" — release this task ships in
  cli            TEXT,                    -- LLM CLI used to run it (dispatch.sh pipeline tasks only)
  model          TEXT,
  model_used     TEXT,
  effort         TEXT,
  prompt_file    TEXT,
  worktree       TEXT,
  branch         TEXT,
  target_task    TEXT,                    -- for kind=merge: the generate task it merges
  tests_status   TEXT,                    -- not run | passed | failed | skipped | no changes made
  context        TEXT,                    -- freeform: what this is, which docs/files are relevant, why it exists
  started_at     TEXT,
  finished_at    TEXT,
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS task_depends (
  task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on  TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on)
);

CREATE TABLE IF NOT EXISTS task_fallback_models (
  task_id   TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  position  INTEGER NOT NULL,
  model     TEXT NOT NULL,
  PRIMARY KEY (task_id, position)
);

-- kind: done | missing | todo — matches the old notes.{done,missing,todo} arrays
CREATE TABLE IF NOT EXISTS task_notes (
  task_id   TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  kind      TEXT NOT NULL CHECK (kind IN ('done', 'missing', 'todo')),
  position  INTEGER NOT NULL,
  text      TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  PRIMARY KEY (task_id, kind, position)
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_target_version ON tasks(target_version);
CREATE INDEX IF NOT EXISTS idx_task_depends_task_id ON task_depends(task_id);

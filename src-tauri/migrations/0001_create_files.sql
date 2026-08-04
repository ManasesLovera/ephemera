CREATE TABLE IF NOT EXISTS files (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    size       BIGINT NOT NULL,
    mime       TEXT,
    data       BYTEA NOT NULL,
    origin     TEXT NOT NULL CHECK (origin IN ('ram', 'disk')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

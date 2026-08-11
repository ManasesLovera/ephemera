# 08 — Database tier (Postgres, via Docker)

## The idea

A third storage tier, alongside RAM and disk: **Postgres**, storing file bytes as
binary (`BYTEA`) rows. Reached from RAM or from disk via an explicit "Save to database"
button — the same one-way, deliberate-action model as persisting to disk. Capped at
**100 MB**, the largest of the three, which is itself a teaching point: a database is
usually backed by its own disk, so its "tier" in the hierarchy is really disk-again, one
layer removed, with structure and query cost added on top.

## Why Postgres, why Docker

Postgres is the natural choice because it is the database most learners will meet
professionally, and `BYTEA` makes the binary-storage story concrete without needing a
blob-specific product. Docker means the database is disposable and reproducible — no
system-wide Postgres install, no version conflicts with anything else on this machine, a
single `docker compose up` to start and `docker compose down -v` to wipe it completely.

Docker 29.6 is already running on this machine with the current user in the `docker`
group (verified 2026-08-04) — no setup needed beyond the compose file below.

## docker-compose.yml

Place at the project root, alongside `crates/`:

```yaml
services:
  postgres:
    image: postgres:17-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: ephemera
      POSTGRES_PASSWORD: ephemera_dev_only
      POSTGRES_DB: ephemera
    ports:
      - "5432:5432"
    volumes:
      - ephemera_pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ephemera"]
      interval: 5s
      timeout: 3s
      retries: 5

volumes:
  ephemera_pgdata:
```

> [!warning]
> `ephemera_dev_only` is a placeholder credential for **local development only** — this
> container is not exposed beyond localhost and holds no real data, but do not reuse this
> password pattern anywhere that matters. If the app ever needs to run on a shared
> machine, move the password to a `.env` file excluded from git (see the `.gitignore`
> note in [`04-tech-stack.md`](04-tech-stack.md)).

Port 5432 was free on this machine as of 2026-08-04. Start it with:

```bash
docker compose up -d
docker compose ps            # wait for "healthy"
```

## Schema

```sql
CREATE TABLE files (
    id           UUID PRIMARY KEY,
    name         TEXT NOT NULL,
    size         BIGINT NOT NULL,      -- logical byte length, for the app's own accounting
    mime         TEXT,
    data         BYTEA NOT NULL,
    origin       TEXT NOT NULL CHECK (origin IN ('ram', 'disk')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Run as a migration on startup (see `sqlx::migrate!` below) rather than a manual script,
so a fresh `docker compose up` plus first launch is enough to get a working database.

### A second honest-numbers lesson, inside the DB tier

`SUM(size)` from the table gives the *logical* bytes stored — the sum of what the user
uploaded. It is **not** what the database actually consumes on disk. `BYTEA` values over
~2 KB get moved to **TOAST** storage with its own compression and page overhead, and the
table itself carries per-row and per-page overhead. Query the real figure with:

```sql
SELECT pg_total_relation_size('files');  -- table + indexes + TOAST, in bytes
```

**Show both numbers in the DB usage meter**, labelled distinctly: "100.2 MB stored"
(logical, what counts against the 100 MB cap) vs. "104.6 MB on disk" (physical, via
`pg_total_relation_size`). This is the same lesson as the RAM-store-vs-process-RSS gap
in [`02-architecture.md`](02-architecture.md), one layer down: **even inside a layer
that itself lives on disk, "how much did I store" and "how much space did that cost"
are different questions.** Enforce the 100 MB cap against the logical sum (`size`
column), since that is the number the user controls and understands; the physical size
is presented as a secondary, informative figure only.

## Rust integration

| Crate | Purpose |
| --- | --- |
| `sqlx` (features: `postgres`, `runtime-tokio-rustls`, `uuid`, `chrono`, `macros`) | Async Postgres client with compile-time-checked queries and a built-in migration runner |

`sqlx` over `tokio-postgres` directly: `sqlx::migrate!` handles the schema above with
zero extra tooling, and its query macros catch a mistyped column at compile time rather
than at a demo in front of a class.

```rust
pub struct DbStore {
    pool: sqlx::PgPool,
}

impl DbStore {
    pub async fn connect(url: &str) -> Result<Self, AppError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn insert(&self, meta: &FileMeta, bytes: Arc<[u8]>, origin: Origin) -> Result<(), AppError> {
        // check assert_fits(current_logical_sum, meta.size, MAX_DB_BYTES) first
        sqlx::query!(
            "INSERT INTO files (id, name, size, mime, data, origin) VALUES ($1, $2, $3, $4, $5, $6)",
            meta.id, meta.name, meta.size as i64, meta.mime, &bytes[..], origin.as_str(),
        ).execute(&self.pool).await?;
        Ok(())
    }
}
```

`DATABASE_URL` for local dev: `postgres://ephemera:ephemera_dev_only@localhost:5432/ephemera`.
Store it in `crates/ephemera-app/.env` (gitignored) and read via `dotenvy` at startup, falling back
to an in-app "Database offline — start it with `docker compose up`" state if the connect
fails, rather than crashing. The database is optional infrastructure; the app's core
lesson (RAM vs. disk) must work with it stopped.

## Core API additions

Plain functions in `crates/ephemera-core`, called directly (no IPC boundary — see
[`02-architecture.md`](02-architecture.md)):

| Function | Args | Returns | Notes |
| --- | --- | --- | --- |
| `save_to_db` | `id`, source (`ram` \| `disk`) | `DbFile` | Enforces 100 MB logical cap |
| `list_db` | — | `Vec<DbFile>` | |
| `delete_from_db` | `id` | `()` | |
| `get_db_status` | — | `DbStatus { connected, logical_bytes, physical_bytes, cap }` | Polled or events; also drives the "offline" banner |

No `load_from_db` — the tier graph is one-way into the database (see
[`01-requirements.md`](01-requirements.md#moving-files-and-the-tier-graph)).

## UI

A third pane/card, reachable via a **"Save to database"** button that appears on every
file card in both the RAM pane and the disk pane (per the tier graph: RAM → DB and
Disk → DB are both valid). The database itself is presented as a compact panel rather
than a third full pane — it is a sink, not a place files are dragged *from* — showing the
same segmented meter pattern as RAM/disk, plus the logical-vs-physical size note.

If the container is not running, the panel shows an offline state with the exact command
to fix it (`docker compose up -d`) rather than a generic error.

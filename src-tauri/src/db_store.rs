use crate::error::{assert_fits, AppError};
use crate::types::{DbFile, FileId, FileMeta, Origin, MAX_DB_BYTES};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub struct DbStore {
    pool: Option<PgPool>,
    offline_reason: Option<String>,
}

impl DbStore {
    pub async fn connect(url: &str) -> Self {
        match PgPoolOptions::new().max_connections(5).acquire_timeout(std::time::Duration::from_secs(3)).connect(url).await {
            Ok(pool) => {
                if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
                    return Self { pool: None, offline_reason: Some(format!("migration failed: {e}")) };
                }
                Self { pool: Some(pool), offline_reason: None }
            }
            Err(e) => Self { pool: None, offline_reason: Some(e.to_string()) },
        }
    }

    pub fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    pub fn offline_reason(&self) -> Option<String> {
        self.offline_reason.clone()
    }

    fn pool(&self) -> Result<&PgPool, AppError> {
        self.pool.as_ref().ok_or_else(|| AppError::DbUnavailable {
            message: self.offline_reason.clone().unwrap_or_else(|| "not connected — run `docker compose up -d`".into()),
        })
    }

    pub async fn logical_bytes(&self) -> Result<u64, AppError> {
        let pool = self.pool()?;
        let row = sqlx::query("SELECT COALESCE(SUM(size), 0)::bigint AS total FROM files")
            .fetch_one(pool)
            .await?;
        Ok(row.try_get::<i64, _>("total").unwrap_or(0).max(0) as u64)
    }

    pub async fn physical_bytes(&self) -> Result<u64, AppError> {
        let pool = self.pool()?;
        let row = sqlx::query("SELECT pg_total_relation_size('files')::bigint AS total")
            .fetch_one(pool)
            .await?;
        Ok(row.try_get::<i64, _>("total").unwrap_or(0).max(0) as u64)
    }

    pub async fn insert(&self, meta: &FileMeta, bytes: &[u8], origin: Origin) -> Result<DbFile, AppError> {
        let pool = self.pool()?;
        let current = self.logical_bytes().await.unwrap_or(0);
        assert_fits(current, meta.size, MAX_DB_BYTES)?;
        let origin_str = match origin {
            Origin::Ram => "ram",
            Origin::Disk => "disk",
            _ => "ram",
        };
        sqlx::query(
            "INSERT INTO files (id, name, size, mime, data, origin, created_at) VALUES ($1, $2, $3, $4, $5, $6, now())",
        )
        .bind(&meta.id)
        .bind(&meta.name)
        .bind(meta.size as i64)
        .bind(&meta.mime)
        .bind(bytes)
        .bind(origin_str)
        .execute(pool)
        .await?;
        Ok(DbFile { meta: meta.clone(), saved_at: now_millis() })
    }

    pub async fn list(&self) -> Result<Vec<DbFile>, AppError> {
        let pool = self.pool()?;
        let rows = sqlx::query(
            "SELECT id, name, size, mime, origin, extract(epoch from created_at)*1000 as created_at FROM files ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let id: String = row.try_get("id").unwrap_or_default();
                let name: String = row.try_get("name").unwrap_or_default();
                let size: i64 = row.try_get("size").unwrap_or(0);
                let mime: String = row.try_get("mime").unwrap_or_default();
                let created_at: f64 = row.try_get("created_at").unwrap_or(0.0);
                DbFile {
                    meta: FileMeta {
                        id,
                        name,
                        size: size.max(0) as u64,
                        mime,
                        created_at: created_at as i64,
                        origin: Origin::Disk,
                    },
                    saved_at: created_at as i64,
                }
            })
            .collect())
    }

    pub async fn remove(&self, id: &FileId) -> Result<(), AppError> {
        let pool = self.pool()?;
        sqlx::query("DELETE FROM files WHERE id = $1").bind(id).execute(pool).await?;
        Ok(())
    }
}

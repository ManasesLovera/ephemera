use crate::error::{assert_fits, AppError};
use crate::types::{DiskFile, FileId, FileMeta, Origin, MAX_DISK_BYTES};
use indexmap::IndexMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Vault {
    root: PathBuf,
    index: IndexMap<FileId, DiskFile>,
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Strips path separators, `..`, and control characters. Empty result is rejected.
pub fn sanitize_filename(name: &str) -> Result<String, AppError> {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\') && !c.is_control())
        .collect();
    let cleaned = cleaned.replace("..", "");
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        return Err(AppError::InvalidName);
    }
    Ok(cleaned)
}

fn unique_dest(root: &Path, name: &str) -> PathBuf {
    let mut candidate = root.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = Path::new(name).extension().and_then(|s| s.to_str());
    for n in 2..10_000 {
        let new_name = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        candidate = root.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    candidate
}

impl Vault {
    pub fn open(root: PathBuf) -> Result<Self, AppError> {
        std::fs::create_dir_all(&root)?;
        let mut v = Self { root, index: IndexMap::new() };
        v.rescan()?;
        Ok(v)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves `name` inside the vault root and asserts the result cannot escape it.
    pub fn resolve_safe(&self, name: &str) -> Result<PathBuf, AppError> {
        let safe_name = sanitize_filename(name)?;
        let candidate = self.root.join(&safe_name);
        let canon_root = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
        // The candidate does not exist yet in the "write new file" case, so canonicalize its parent.
        let parent_canon = candidate
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .unwrap_or_else(|| self.root.clone());
        if parent_canon != canon_root {
            return Err(AppError::PathEscape);
        }
        Ok(candidate)
    }

    pub fn total_bytes(&self) -> u64 {
        self.index.values().map(|f| f.meta.size).sum()
    }

    pub fn list(&self) -> Vec<DiskFile> {
        self.index.values().cloned().collect()
    }

    pub fn assert_room_for(&self, size: u64) -> Result<(), AppError> {
        assert_fits(self.total_bytes(), size, MAX_DISK_BYTES)
    }

    /// Writes bytes to a new file inside the vault (used by RAM->disk persist).
    /// Caller has already validated quota and filename.
    pub fn write_new_path(&self, name: &str) -> Result<PathBuf, AppError> {
        let safe_name = sanitize_filename(name)?;
        Ok(unique_dest(&self.root, &safe_name))
    }

    pub fn register(&mut self, meta: FileMeta) {
        self.index.insert(meta.id.clone(), DiskFile { meta, persisted_at: now_millis() });
    }

    pub fn remove(&mut self, id: &str) -> Result<(), AppError> {
        let entry = self.index.shift_remove(id).ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
        let path = self.root.join(&entry.meta.name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn get_path(&self, id: &str) -> Result<PathBuf, AppError> {
        let entry = self.index.get(id).ok_or_else(|| AppError::NotFound { id: id.to_string() })?;
        Ok(self.root.join(&entry.meta.name))
    }

    /// Re-derives the index from the folder's actual contents — the vault is the
    /// source of truth, not our in-memory cache.
    pub fn rescan(&mut self) -> Result<(), AppError> {
        let mut fresh = IndexMap::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue; // sidecar/hidden files excluded from accounting
            }
            let metadata = entry.metadata()?;
            let size = metadata.len();
            // Reuse a prior id if we already knew this file by name, else mint one.
            let existing = self.index.values().find(|f| f.meta.name == name);
            let (id, created_at, persisted_at) = match existing {
                Some(f) => (f.meta.id.clone(), f.meta.created_at, f.persisted_at),
                None => (uuid::Uuid::new_v4().to_string(), now_millis(), now_millis()),
            };
            let mime = mime_guess::from_path(&name).first_or_octet_stream().to_string();
            fresh.insert(
                id.clone(),
                DiskFile {
                    meta: FileMeta { id, name, size, mime, created_at, origin: Origin::Disk },
                    persisted_at,
                },
            );
        }
        self.index = fresh;
        Ok(())
    }
}

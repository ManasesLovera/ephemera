use crate::error::{assert_fits, AppError};
use crate::types::{FileId, FileMeta, MAX_RAM_BYTES, MAX_SINGLE_FILE};
use indexmap::IndexMap;
use std::sync::Arc;

pub struct RamFile {
    pub meta: FileMeta,
    pub bytes: Arc<[u8]>,
}

#[derive(Default)]
pub struct RamStore {
    files: IndexMap<FileId, RamFile>,
}

impl RamStore {
    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|f| f.meta.size).sum()
    }

    pub fn list(&self) -> Vec<FileMeta> {
        self.files.values().map(|f| f.meta.clone()).collect()
    }

    pub fn get(&self, id: &str) -> Option<&RamFile> {
        self.files.get(id)
    }

    pub fn insert(&mut self, meta: FileMeta, bytes: Arc<[u8]>) -> Result<(), AppError> {
        if meta.size > MAX_SINGLE_FILE {
            return Err(AppError::FileTooLarge {
                size: meta.size,
                cap: MAX_SINGLE_FILE,
            });
        }
        assert_fits(self.total_bytes(), meta.size, MAX_RAM_BYTES)?;
        self.files.insert(meta.id.clone(), RamFile { meta, bytes });
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<RamFile, AppError> {
        self.files
            .shift_remove(id)
            .ok_or_else(|| AppError::NotFound { id: id.to_string() })
    }

    pub fn flush(&mut self) {
        self.files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Origin;

    fn meta(id: &str, size: u64) -> FileMeta {
        FileMeta {
            id: id.to_string(),
            name: format!("{id}.bin"),
            size,
            mime: "application/octet-stream".to_string(),
            created_at: 0,
            origin: Origin::Upload,
        }
    }

    #[test]
    fn insert_and_total_bytes() {
        let mut store = RamStore::default();
        store
            .insert(
                meta("a", 1024),
                Arc::from(vec![0u8; 1024].into_boxed_slice()),
            )
            .unwrap();
        assert_eq!(store.total_bytes(), 1024);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn rejects_file_over_ram_cap() {
        let mut store = RamStore::default();
        let err = store.insert(
            meta("big", MAX_RAM_BYTES + 1),
            Arc::from(vec![].into_boxed_slice()),
        );
        assert!(matches!(err, Err(AppError::FileTooLarge { .. })));
    }

    #[test]
    fn rejects_when_quota_would_be_exceeded() {
        let mut store = RamStore::default();
        store
            .insert(
                meta("a", MAX_RAM_BYTES - 100),
                Arc::from(vec![].into_boxed_slice()),
            )
            .unwrap();
        let err = store.insert(meta("b", 200), Arc::from(vec![].into_boxed_slice()));
        assert!(matches!(err, Err(AppError::QuotaExceeded { .. })));
    }

    #[test]
    fn flush_drops_everything() {
        let mut store = RamStore::default();
        store
            .insert(meta("a", 10), Arc::from(vec![].into_boxed_slice()))
            .unwrap();
        store.flush();
        assert_eq!(store.total_bytes(), 0);
        assert!(store.list().is_empty());
    }

    #[test]
    fn deleting_one_file_frees_its_quota() {
        let mut store = RamStore::default();
        store
            .insert(
                meta("a", MAX_RAM_BYTES - 100),
                Arc::from(vec![].into_boxed_slice()),
            )
            .unwrap();
        store.remove("a").unwrap();
        assert_eq!(store.total_bytes(), 0);
        // now the full cap is available again
        store
            .insert(
                meta("b", MAX_RAM_BYTES),
                Arc::from(vec![].into_boxed_slice()),
            )
            .unwrap();
    }

    #[test]
    fn remove_missing_id_is_not_found() {
        let mut store = RamStore::default();
        let err = store.remove("nope");
        assert!(matches!(err, Err(AppError::NotFound { .. })));
    }
}

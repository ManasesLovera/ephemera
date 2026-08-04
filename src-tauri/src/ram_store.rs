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
            return Err(AppError::FileTooLarge { size: meta.size, cap: MAX_SINGLE_FILE });
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

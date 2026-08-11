//! On-disk persistence for the small slice of app config that must survive
//! restarts — currently just the vault path (`docs/01-requirements.md`,
//! "Configuration": "MUST persist the vault path between runs").
//!
//! The file lives in the OS config dir (`dirs::config_dir()`), never inside
//! the vault: mixing app state into the vault would corrupt the disk-usage
//! accounting and muddy the metaphor (same section, "SHOULD keep app config in
//! the OS config dir"). Only metadata lives here — a path string and a
//! timestamp, never file bytes — so the vault rules in `CLAUDE.md` are
//! untouched.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// What gets persisted. `updated_at` is informational only (when the path was
/// last set); nothing makes decisions from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedConfig {
    pub vault_path: String,
    pub updated_at: i64,
}

/// `<os-config-dir>/ephemera/config.json` — e.g. `~/.config/ephemera/config.json`
/// on Linux. `None` when the OS reports no config dir (effectively never on
/// desktop Linux/macOS/Windows); callers treat that like a failed write.
pub fn config_file_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("ephemera").join("config.json"))
}

/// The persisted config, or `None` when the file is absent, unreadable, or
/// corrupt. `None` must always mean "behave like first run", never a crash.
pub fn load() -> Option<PersistedConfig> {
    load_from(&config_file_path()?)
}

/// Best-effort write of the vault path, called by `set_vault_path` after the
/// in-memory switch has already succeeded. Errors are returned for the caller
/// to log, but must never fail the vault switch itself.
pub(crate) fn save_vault_path(vault_path: &str) -> std::io::Result<()> {
    let path = config_file_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no OS config dir available")
    })?;
    save_to(
        &path,
        &PersistedConfig {
            vault_path: vault_path.to_string(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        },
    )
}

fn load_from(path: &Path) -> Option<PersistedConfig> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_to(path: &Path, config: &PersistedConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // fsync, same rule as the vault: a "persisted" path that only reached the
    // page cache could silently revert to the default after a crash.
    let mut file = std::fs::File::create(path)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_save_then_load() {
        let dir = tempfile::tempdir().unwrap();
        // Nested path also proves save_to creates missing parent dirs.
        let path = dir.path().join("ephemera").join("config.json");
        let config = PersistedConfig {
            vault_path: "/tmp/my-custom-vault".to_string(),
            updated_at: 1_754_956_800_000,
        };
        save_to(&path, &config).unwrap();
        assert_eq!(load_from(&path), Some(config));
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_from(&dir.path().join("config.json")), None);
    }

    #[test]
    fn load_corrupt_json_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"this is not json").unwrap();
        assert_eq!(load_from(&path), None);
    }

    #[test]
    fn load_wrong_shape_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, br#"{"vault_path": "/tmp/x"}"#).unwrap(); // no updated_at
        assert_eq!(load_from(&path), None);
    }

    /// The two "launches" of the manual verification, back to back through the
    /// real public entry points: launch 1 sets the vault path (write side of
    /// `set_vault_path`), launch 2 reads it back at startup — same config dir,
    /// and the file provably lives under the OS config dir, not the vault.
    #[test]
    fn persisted_path_survives_simulated_relaunch() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        let result = std::panic::catch_unwind(|| {
            save_vault_path("/tmp/launch-1-vault").unwrap();

            let config_path = config_file_path().unwrap();
            assert!(config_path.starts_with(dir.path()));
            assert!(config_path.is_file());

            let reloaded = load().expect("second launch must read the persisted path");
            assert_eq!(reloaded.vault_path, "/tmp/launch-1-vault");
            assert!(reloaded.updated_at > 0);
        });
        std::env::remove_var("XDG_CONFIG_HOME");
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }
}

//! Small persisted-settings file for shell-level UI preferences.
//!
//! As of this writing (GAP-I18N) neither GAP-DARK-MODE nor GAP-VAULT-PERSIST has
//! landed on `main` — there is no existing persisted-config plumbing to reuse, so
//! this introduces a minimal, extensible `settings.json` that those gaps can add
//! keys to later rather than each growing its own separate file. Unknown keys are
//! preserved on write is out of scope for now (only `language` exists); if a
//! second key shows up, switch this to a `serde_json::Value` merge instead of a
//! flat struct so a newer binary doesn't clobber keys an older one didn't know
//! about.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: default_language(),
        }
    }
}

fn default_language() -> String {
    "en".to_string()
}

/// `$XDG_CONFIG_HOME/com.ephemera.app/settings.json`, falling back to
/// `~/.config/com.ephemera.app/settings.json` — the config-file counterpart of
/// `default_vault_path`'s use of `XDG_DATA_HOME` in `main.rs`.
fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("com.ephemera.app").join("settings.json"))
}

/// Best-effort load: a missing or corrupt file just yields defaults rather than
/// failing startup over a UI preference.
pub fn load() -> Settings {
    settings_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Best-effort save: failures (e.g. read-only home) are silently ignored — losing
/// a language preference isn't worth surfacing an error banner over.
pub fn save(settings: &Settings) {
    let Some(path) = settings_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
}

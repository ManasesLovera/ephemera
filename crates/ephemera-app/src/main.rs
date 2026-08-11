//! Ephemera — Slint application shell.
//!
//! This binary replaces the Tauri + React shell. The UI and the core run in one
//! process, one address space: `ephemera_core::state::AppState` is the single
//! authoritative store, and `model::ShellState` projects it into Slint properties
//! directly. There is no IPC and no serialization boundary — which is exactly why
//! the metadata-only rule from `CLAUDE.md` has to be enforced in code (see
//! `model.rs`) rather than being handed a process boundary to do it for us.

slint::include_modules!();

mod model;
mod settings;

use ephemera_core::state::AppState;
use model::{describe_error, ShellState};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Same vault location the Tauri app uses (identifier `com.ephemera.app` from
/// `tauri.conf.json`): `$XDG_DATA_HOME/com.ephemera.app/vault` on Linux, i.e.
/// `~/.local/share/com.ephemera.app/vault` by default. Sharing the path keeps the
/// disk store continuous across the migration.
fn default_vault_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("com.ephemera.app").join("vault")
}

/// Vault path for this launch: the user's persisted choice when it still
/// points at an existing, writable directory; otherwise the default
/// (docs/01-requirements.md, "Configuration": "MUST persist the vault path
/// between runs"). The persisted file is only read here, never rewritten —
/// when the chosen folder is temporarily gone (e.g. an unplugged drive) the
/// app falls back for this run but keeps the choice on disk for the next one.
fn initial_vault_path() -> PathBuf {
    let persisted = ephemera_core::config_file::load().map(|cfg| cfg.vault_path);
    resolve_vault_path(persisted, default_vault_path())
}

/// Pure decision behind `initial_vault_path`, split out for testing. A
/// persisted path is unusable when it is relative, no longer a directory, or
/// not writable (`readonly()` is the no-write-bits heuristic — a read-only
/// mount with write bits set still slips through, but the first real write
/// then fails cleanly, which the vault already handles). Unusable means "use
/// the default for this launch", never a crash (docs/01 Non-functional: the
/// vault being deleted or made read-only must not take the app down).
fn resolve_vault_path(persisted: Option<String>, default: PathBuf) -> PathBuf {
    let Some(candidate) = persisted.map(PathBuf::from) else {
        return default;
    };
    let usable = candidate.is_absolute()
        && std::fs::metadata(&candidate)
            .map(|meta| meta.is_dir() && !meta.permissions().readonly())
            .unwrap_or(false);
    if usable {
        candidate
    } else {
        eprintln!(
            "ephemera: persisted vault path {} is missing or not writable; \
             falling back to default {}",
            candidate.display(),
            default.display()
        );
        default
    }
}

/// Walks upward from `start`, returning the first ancestor directory containing
/// `filename`. Carried over from the Tauri build (`src-tauri/src/lib.rs`, commit
/// `0e2fc7a`): anchoring on the executable's own location, rather than the
/// process's current working directory, means `.env` and `gcs-key.json` resolve
/// the same way whether launched via `cargo run` (exe under `target/debug`) or as
/// a standalone release binary run from anywhere.
fn find_upwards(start: &Path, filename: &str) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(filename);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

fn main() -> Result<(), slint::PlatformError> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let env_path = exe_dir.as_deref().and_then(|d| find_upwards(d, ".env"));
    match &env_path {
        Some(path) => {
            let _ = dotenvy::from_path(path);
        }
        None => {
            let _ = dotenvy::dotenv();
        }
    }
    // GCS_KEY_PATH (whether from .env or its default) is conventionally a bare
    // filename meant to sit next to .env — resolve it against that directory
    // rather than the process's current working directory, which varies by
    // launch method (`cargo run` vs. a standalone release binary run from
    // anywhere).
    let config_dir = env_path
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .or(exe_dir)
        .unwrap_or_else(|| PathBuf::from("."));

    // A multi-threaded tokio runtime so the core's async DB/cloud stores (and the
    // slow 3 s usage refresher) keep progressing while the Slint event loop runs
    // on the main thread.
    let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");

    let vault_path = initial_vault_path();
    let (core, db_status, db_files, cloud_status, cloud_files) = rt.block_on(async {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://ephemera:ephemera_dev_only@localhost:5432/ephemera".to_string()
        });
        let db = ephemera_core::db_store::DbStore::connect(&database_url).await;

        let gcs_key_path =
            std::env::var("GCS_KEY_PATH").unwrap_or_else(|_| "gcs-key.json".to_string());
        let gcs_key_path = if Path::new(&gcs_key_path).is_absolute() {
            gcs_key_path
        } else {
            config_dir
                .join(&gcs_key_path)
                .to_string_lossy()
                .into_owned()
        };
        let gcs_bucket =
            std::env::var("GCS_BUCKET").unwrap_or_else(|_| "ephemera-vault".to_string());
        let cloud = ephemera_core::cloud_store::CloudStore::load(&gcs_key_path, gcs_bucket);

        let state = Arc::new(AppState::new(vault_path.clone(), db, cloud));
        ephemera_core::state::spawn_slow_metrics_refresher(state.clone());

        // Bootstrap the db/cloud snapshots once up front so the sink panels have
        // an honest connected/offline answer before the 5 s poller's first tick.
        let db_status = ephemera_core::get_db_status(&state).await.ok();
        let db_files = ephemera_core::list_db(&state).await.ok();
        let cloud_status = ephemera_core::get_cloud_status(&state).await.ok();
        let cloud_files = ephemera_core::list_cloud(&state).await.ok();
        (state, db_status, db_files, cloud_status, cloud_files)
    });

    let window = AppWindow::new()?;

    // Bundled-translation language: must be selected after the first component is
    // created (see slint::select_bundled_translation docs). An unrecognized/corrupt
    // saved value or a select error just falls back to the compiled-in default
    // ("en") rather than failing startup over a UI preference.
    let saved_settings = settings::load();
    if slint::select_bundled_translation(&saved_settings.language).is_ok() {
        window.set_current_language(saved_settings.language.clone().into());
    }

    let shared = ShellState::new(core, vault_path.to_string_lossy().to_string());

    window
        .global::<Theme>()
        .set_dark_mode(saved_settings.dark_mode);

    // Initial synchronous refresh (RAM/disk/vault are in-process and cheap), plus
    // the seeded db/cloud snapshots.
    shared.refresh_ram(&window);
    shared.refresh_disk(&window);
    shared.push_vault_path(&window);
    shared.apply_db(&window, db_status, db_files);
    shared.apply_cloud(&window, cloud_status, cloud_files);

    // Toggle dark mode and persist preference to settings.json.
    {
        let weak = window.as_weak();
        let language = saved_settings.language.clone();
        window.on_toggle_dark_mode(move || {
            if let Some(window) = weak.upgrade() {
                let theme = window.global::<Theme>();
                let new_mode = !theme.get_dark_mode();
                theme.set_dark_mode(new_mode);
                settings::save(&settings::Settings {
                    language: language.clone(),
                    dark_mode: new_mode,
                });
            }
        });
    }

    // Slint → Rust callback. The old React `refreshAll` zustand action crossed
    // IPC; this is a plain Rust closure running in the same process.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_refresh_all(move || {
            if let Some(window) = weak.upgrade() {
                shared.refresh_all(&window);
            }
        });
    }

    // Dismiss the error banner.
    {
        let weak = window.as_weak();
        window.on_clear_error(move || {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
            }
        });
    }

    // Language toggle: switch the bundled translation immediately (all @tr()
    // bindings re-evaluate on their own — no manual re-push of translated
    // properties needed) and persist the choice for the next launch.
    {
        let weak = window.as_weak();
        window.on_switch_language(move |lang| {
            if let Some(window) = weak.upgrade() {
                if slint::select_bundled_translation(lang.as_str()).is_ok() {
                    window.set_current_language(lang.clone());
                    let dark_mode = window.global::<Theme>().get_dark_mode();
                    settings::save(&settings::Settings {
                        language: lang.to_string(),
                        dark_mode,
                    });
                } else {
                    window.set_error_message(format!("Unknown language: {}", lang.as_str()).into());
                }
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_upload_ram_files(move || {
            if let Some(files) = rfd::FileDialog::new().pick_files() {
                let shared = shared.clone();
                let weak = weak.clone();
                rt_handle.spawn(async move {
                    let mut last_err = None;
                    for path in files {
                        if let Err(e) =
                            ephemera_core::upload_to_ram(&shared.core, &path.to_string_lossy())
                                .await
                        {
                            last_err = Some(e);
                            break;
                        }
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            if let Some(e) = last_err {
                                window.set_error_message(describe_error(&e).into());
                            } else {
                                window.set_error_message("".into());
                            }
                            shared.refresh_ram(&window);
                        }
                    });
                });
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_stream_upload_to_disk(move || {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                if let Some(window) = weak.upgrade() {
                    window.set_error_message("".into());
                    let res = ephemera_core::stream_upload_to_disk(
                        &shared.core,
                        &path.to_string_lossy(),
                        |_| {},
                    );
                    match res {
                        Ok(report) => {
                            shared.set_stream_report(&window, report);
                            shared.refresh_disk(&window);
                        }
                        Err(e) => {
                            window.set_error_message(describe_error(&e).into());
                        }
                    }
                }
            }
        });
    }

    {
        let weak = window.as_weak();
        window.on_close_stream_report(move || {
            if let Some(window) = weak.upgrade() {
                window.set_show_stream_report(false);
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_flush_ram(move || {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
                ephemera_core::flush_ram(&shared.core);
                shared.refresh_ram(&window);
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_persist_to_disk(move |id| {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
                match ephemera_core::persist_to_disk(&shared.core, id.as_str()) {
                    Ok(_) => {
                        shared.refresh_ram(&window);
                        shared.refresh_disk(&window);
                    }
                    Err(e) => {
                        window.set_error_message(describe_error(&e).into());
                    }
                }
            }
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_save_to_db(move |id| {
            let shared = shared.clone();
            let weak = weak.clone();
            let id = id.to_string();
            rt_handle.spawn(async move {
                let res = ephemera_core::save_to_db(&shared.core, &id, "ram").await;
                let db_status = ephemera_core::get_db_status(&shared.core).await.ok();
                let db_files = ephemera_core::list_db(&shared.core).await.ok();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        match res {
                            Ok(_) => {
                                shared.refresh_ram(&window);
                                shared.apply_db(&window, db_status, db_files);
                            }
                            Err(e) => {
                                window.set_error_message(describe_error(&e).into());
                            }
                        }
                    }
                });
            });
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_save_to_cloud(move |id| {
            let shared = shared.clone();
            let weak = weak.clone();
            let id = id.to_string();
            rt_handle.spawn(async move {
                let res = ephemera_core::save_to_cloud(&shared.core, &id, "ram").await;
                let cloud_status = ephemera_core::get_cloud_status(&shared.core).await.ok();
                let cloud_files = ephemera_core::list_cloud(&shared.core).await.ok();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        match res {
                            Ok(_) => {
                                shared.refresh_ram(&window);
                                shared.apply_cloud(&window, cloud_status, cloud_files);
                            }
                            Err(e) => {
                                window.set_error_message(describe_error(&e).into());
                            }
                        }
                    }
                });
            });
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_delete_from_ram(move |id| {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
                if let Err(e) = ephemera_core::delete_from_ram(&shared.core, id.as_str()) {
                    window.set_error_message(describe_error(&e).into());
                } else {
                    shared.refresh_ram(&window);
                }
            }
        });
    }

    // "Open folder": reveal the vault directory in the OS file manager. The Tauri
    // build used `tauri_plugin_opener`; here we shell out to the platform opener
    // on the same real folder (`get_vault_path`).
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_reveal_vault(move || {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
                let path = ephemera_core::get_vault_path(&shared.core);
                if let Err(e) = open_in_file_manager(&path) {
                    window.set_error_message(describe_error(&e).into());
                }
            }
        });
    }

    // "Rescan": re-derive the vault index from the folder contents.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_rescan_vault(move || {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
                match ephemera_core::rescan_vault(&shared.core) {
                    Ok(_) => {
                        shared.refresh_disk(&window);
                    }
                    Err(e) => {
                        window.set_error_message(describe_error(&e).into());
                    }
                }
            }
        });
    }

    // Disk → database: one-way sink, never back to RAM. Runs on the tokio runtime
    // because the Postgres store is async; marshal the result back via
    // invoke_from_event_loop.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_save_disk_to_db(move |id| {
            let shared = shared.clone();
            let weak = weak.clone();
            let id = id.to_string();
            rt_handle.spawn(async move {
                let res = ephemera_core::save_to_db(&shared.core, &id, "disk").await;
                let db_status = ephemera_core::get_db_status(&shared.core).await.ok();
                let db_files = ephemera_core::list_db(&shared.core).await.ok();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        match res {
                            Ok(_) => {
                                shared.apply_db(&window, db_status, db_files);
                            }
                            Err(e) => {
                                window.set_error_message(describe_error(&e).into());
                            }
                        }
                    }
                });
            });
        });
    }

    // Disk → cloud: one-way sink, never back to RAM. Same async marshalling as
    // the database path above.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_save_disk_to_cloud(move |id| {
            let shared = shared.clone();
            let weak = weak.clone();
            let id = id.to_string();
            rt_handle.spawn(async move {
                let res = ephemera_core::save_to_cloud(&shared.core, &id, "disk").await;
                let cloud_status = ephemera_core::get_cloud_status(&shared.core).await.ok();
                let cloud_files = ephemera_core::list_cloud(&shared.core).await.ok();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        match res {
                            Ok(_) => {
                                shared.apply_cloud(&window, cloud_status, cloud_files);
                            }
                            Err(e) => {
                                window.set_error_message(describe_error(&e).into());
                            }
                        }
                    }
                });
            });
        });
    }

    // Delete the real file from the vault folder (the store's `remove` unlinks).
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        window.on_delete_from_disk(move |id| {
            if let Some(window) = weak.upgrade() {
                window.set_error_message("".into());
                if let Err(e) = ephemera_core::delete_from_disk(&shared.core, id.as_str()) {
                    window.set_error_message(describe_error(&e).into());
                } else {
                    shared.refresh_disk(&window);
                }
            }
        });
    }

    // Delete actions for DB and Cloud sink panels.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_delete_db_file(move |id| {
            let shared = shared.clone();
            let weak = weak.clone();
            let id_str = id.to_string();
            rt_handle.spawn(async move {
                if ephemera_core::delete_from_db(&shared.core, &id_str)
                    .await
                    .is_ok()
                {
                    let db_status = ephemera_core::get_db_status(&shared.core).await.ok();
                    let db_files = ephemera_core::list_db(&shared.core).await.ok();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            shared.apply_db(&window, db_status, db_files);
                        }
                    });
                }
            });
        });
    }

    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let rt_handle = rt.handle().clone();
        window.on_delete_cloud_file(move |id| {
            let shared = shared.clone();
            let weak = weak.clone();
            let id_str = id.to_string();
            rt_handle.spawn(async move {
                if ephemera_core::delete_from_cloud(&shared.core, &id_str)
                    .await
                    .is_ok()
                {
                    let cloud_status = ephemera_core::get_cloud_status(&shared.core).await.ok();
                    let cloud_files = ephemera_core::list_cloud(&shared.core).await.ok();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            shared.apply_cloud(&window, cloud_status, cloud_files);
                        }
                    });
                }
            });
        });
    }

    // 4 Hz metrics: the core sampler runs on its own thread; marshal each tick
    // onto the UI thread via invoke_from_event_loop (the canonical Slint bridge
    // from worker to UI thread). No panics on this path: the closure only locks
    // via `if let Ok`, upgrades a weak handle, and sets properties.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        let _sampler = ephemera_core::spawn_sampler(shared.core.clone(), move |metrics| {
            let shared = shared.clone();
            let weak = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(window) = weak.upgrade() {
                    shared.push_metrics(&window, &metrics);
                }
            });
        });
    }

    // 5 s db/cloud poller, mirroring the React app's `setInterval(…, 5000)`.
    {
        let shared = shared.clone();
        let weak = window.as_weak();
        rt.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let db_status = ephemera_core::get_db_status(&shared.core).await.ok();
                let db_files = ephemera_core::list_db(&shared.core).await.ok();
                let cloud_status = ephemera_core::get_cloud_status(&shared.core).await.ok();
                let cloud_files = ephemera_core::list_cloud(&shared.core).await.ok();
                let shared = shared.clone();
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak.upgrade() {
                        shared.apply_db(&window, db_status, db_files);
                        shared.apply_cloud(&window, cloud_status, cloud_files);
                    }
                });
            }
        });
    }

    window.run()
}

/// Open a folder in the OS file manager (the Slint replacement for Tauri's
/// `tauri_plugin_opener::reveal_item_in_dir`). Each platform gets its own opener;
/// spawning fails only if the command itself can't start.
fn open_in_file_manager(path: &std::path::Path) -> Result<(), ephemera_core::error::AppError> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };

    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) => Err(ephemera_core::error::AppError::Io {
            message: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default() -> PathBuf {
        PathBuf::from("/nonexistent-ephemera-default")
    }

    #[test]
    fn resolve_prefers_persisted_when_usable() {
        let dir = tempfile::tempdir().unwrap();
        let persisted = dir.path().to_string_lossy().into_owned();
        assert_eq!(resolve_vault_path(Some(persisted), default()), dir.path());
    }

    #[test]
    fn resolve_falls_back_when_nothing_persisted() {
        assert_eq!(resolve_vault_path(None, default()), default());
    }

    #[test]
    fn resolve_falls_back_when_persisted_missing() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir
            .path()
            .join("deleted-vault")
            .to_string_lossy()
            .into_owned();
        assert_eq!(resolve_vault_path(Some(gone), default()), default());
    }

    #[test]
    fn resolve_falls_back_when_persisted_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        let persisted = file.to_string_lossy().into_owned();
        assert_eq!(resolve_vault_path(Some(persisted), default()), default());
    }

    #[test]
    fn resolve_falls_back_when_persisted_is_relative() {
        let relative = "definitely-not-a-real-relative-vault-path".to_string();
        assert_eq!(resolve_vault_path(Some(relative), default()), default());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_falls_back_when_persisted_readonly() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let persisted = dir.path().to_string_lossy().into_owned();
        let resolved = resolve_vault_path(Some(persisted), default());
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(resolved, default());
    }
}

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

use ephemera_core::state::AppState;
use model::{describe_error, ShellState};
use std::path::PathBuf;
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

fn main() -> Result<(), slint::PlatformError> {
    // A multi-threaded tokio runtime so the core's async DB/cloud stores (and the
    // slow 3 s usage refresher) keep progressing while the Slint event loop runs
    // on the main thread.
    let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");

    let vault_path = default_vault_path();
    let (core, db_status, db_files, cloud_status, cloud_files) = rt.block_on(async {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://ephemera:ephemera_dev_only@localhost:5432/ephemera".to_string()
        });
        let db = ephemera_core::db_store::DbStore::connect(&database_url).await;

        let gcs_key_path =
            std::env::var("GCS_KEY_PATH").unwrap_or_else(|_| "gcs-key.json".to_string());
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
    let shared = ShellState::new(core, vault_path.to_string_lossy().to_string());

    // Initial synchronous refresh (RAM/disk/vault are in-process and cheap), plus
    // the seeded db/cloud snapshots.
    shared.refresh_ram(&window);
    shared.refresh_disk(&window);
    shared.push_vault_path(&window);
    shared.apply_db(&window, db_status, db_files);
    shared.apply_cloud(&window, cloud_status, cloud_files);

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
                        Ok(_report) => {
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

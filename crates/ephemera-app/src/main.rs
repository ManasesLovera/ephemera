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
use model::ShellState;
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

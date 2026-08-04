use crate::cloud_store::CloudStore;
use crate::db_store::DbStore;
use crate::ram_store::RamStore;
use crate::types::Config;
use crate::vault::Vault;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub ram: Mutex<RamStore>,
    pub vault: Mutex<Vault>,
    pub db: Arc<DbStore>,
    pub cloud: Arc<CloudStore>,
    pub config: Mutex<Config>,
    // Cached so the 4 Hz metrics sampler never blocks on a network/DB round trip.
    // Refreshed by an explicit poller — see spawn_slow_metrics_refresher below.
    db_logical_cached: AtomicU64,
    db_physical_cached: AtomicU64,
    cloud_bytes_cached: AtomicU64,
}

impl AppState {
    pub fn new(vault_path: PathBuf, db: DbStore, cloud: CloudStore) -> Self {
        let vault = Vault::open(vault_path.clone()).expect("vault must be openable");
        Self {
            ram: Mutex::new(RamStore::default()),
            vault: Mutex::new(vault),
            db: Arc::new(db),
            cloud: Arc::new(cloud),
            config: Mutex::new(Config {
                vault_path: vault_path.to_string_lossy().to_string(),
                throttle_ms_per_chunk: 0,
            }),
            db_logical_cached: AtomicU64::new(0),
            db_physical_cached: AtomicU64::new(0),
            cloud_bytes_cached: AtomicU64::new(0),
        }
    }

    pub fn db_usage_cached(&self) -> (u64, u64) {
        (self.db_logical_cached.load(Ordering::Relaxed), self.db_physical_cached.load(Ordering::Relaxed))
    }

    pub fn cloud_usage_cached(&self) -> u64 {
        self.cloud_bytes_cached.load(Ordering::Relaxed)
    }
}

/// Polls DB/cloud usage on a slower cadence (they involve network round trips) and
/// caches the result for the fast metrics sampler to read without blocking.
pub fn spawn_slow_metrics_refresher(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            if let Ok(logical) = state.db.logical_bytes().await {
                state.db_logical_cached.store(logical, Ordering::Relaxed);
            }
            if let Ok(physical) = state.db.physical_bytes().await {
                state.db_physical_cached.store(physical, Ordering::Relaxed);
            }
            if let Ok(bytes) = state.cloud.bytes_used().await {
                state.cloud_bytes_cached.store(bytes, Ordering::Relaxed);
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    });
}

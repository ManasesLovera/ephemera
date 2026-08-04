use crate::state::AppState;
use crate::types::Metrics;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
use tauri::{AppHandle, Emitter};

/// Latest sampled process RSS, in bytes, updated by the sampler thread.
/// A plain atomic rather than routing through AppState's mutexes — this value is read
/// far more often (every metrics tick) than the stores mutate, and it must never block.
pub static LAST_RSS_BYTES: AtomicU64 = AtomicU64::new(0);

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Sums RSS across the whole process tree rooted at `root_pid`. WebKitGTK forks child
/// processes for web content on Linux; reporting only our own PID would badly understate
/// what the app actually costs.
fn tree_rss(sys: &System, root_pid: Pid) -> (u64, usize) {
    let mut total = 0u64;
    let mut count = 0usize;
    let mut stack = vec![root_pid];
    let mut seen = std::collections::HashSet::new();
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some(proc_) = sys.process(pid) {
            total += proc_.memory();
            count += 1;
        }
        for (child_pid, child) in sys.processes() {
            if child.parent() == Some(pid) {
                stack.push(*child_pid);
            }
        }
    }
    (total, count)
}

pub fn spawn_sampler(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        let pid = Pid::from_u32(std::process::id());
        loop {
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let (rss, count) = tree_rss(&sys, pid);
            LAST_RSS_BYTES.store(rss, Ordering::Relaxed);

            let ram_bytes = state.ram.lock().unwrap().total_bytes();
            let disk_bytes = state.vault.lock().unwrap().total_bytes();
            let (db_bytes, db_physical) = state.db_usage_cached();
            let cloud_bytes = state.cloud_usage_cached();

            let metrics = Metrics {
                ts: now_millis(),
                ram_store_bytes: ram_bytes,
                ram_cap: crate::types::MAX_RAM_BYTES,
                disk_store_bytes: disk_bytes,
                disk_cap: crate::types::MAX_DISK_BYTES,
                db_store_bytes: db_bytes,
                db_cap: crate::types::MAX_DB_BYTES,
                db_physical_bytes: db_physical,
                cloud_store_bytes: cloud_bytes,
                cloud_cap: crate::types::MAX_CLOUD_BYTES,
                process_rss_bytes: rss,
                process_count: count,
            };
            let _ = app.emit("metrics://tick", &metrics);
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
}

pub fn sample_rss_now() -> u64 {
    LAST_RSS_BYTES.load(Ordering::Relaxed)
}

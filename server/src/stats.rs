//! System stats tracking with 24h peak values

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tokio::sync::Mutex;

struct PeakStats {
    memory_bytes: AtomicU64,
    cpu_percent: AtomicU64, // stored as percent * 100 (e.g., 5025 = 50.25%)
    recorded_at: Mutex<chrono::DateTime<chrono::Utc>>,
}

static PEAK: OnceLock<PeakStats> = OnceLock::new();

fn peak() -> &'static PeakStats {
    PEAK.get_or_init(|| PeakStats {
        memory_bytes: AtomicU64::new(0),
        cpu_percent: AtomicU64::new(0),
        recorded_at: Mutex::new(chrono::Utc::now()),
    })
}

/// Start background task that samples stats every 30s and tracks 24h peaks
pub fn init() {
    let _ = peak(); // ensure initialized
    tokio::spawn(async {
        let mut sys = sysinfo::System::new();
        let pid = sysinfo::Pid::from_u32(std::process::id());

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;

            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

            if let Some(proc) = sys.process(pid) {
                let mem = proc.memory();
                let cpu = (proc.cpu_usage() as f64 * 100.0) as u64;

                let p = peak();

                // Reset peaks if older than 24h
                {
                    let mut ts = p.recorded_at.lock().await;
                    if chrono::Utc::now() - *ts > chrono::Duration::hours(24) {
                        p.memory_bytes.store(0, Ordering::Relaxed);
                        p.cpu_percent.store(0, Ordering::Relaxed);
                        *ts = chrono::Utc::now();
                    }
                }

                // Update peaks
                p.memory_bytes.fetch_max(mem, Ordering::Relaxed);
                p.cpu_percent.fetch_max(cpu, Ordering::Relaxed);
            }
        }
    });
}

/// Get current and peak stats
pub async fn get_stats() -> serde_json::Value {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());

    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.refresh_memory();

    let (proc_mem, proc_cpu) = sys
        .process(pid)
        .map(|p| (p.memory(), p.cpu_usage()))
        .unwrap_or((0, 0.0));

    let p = peak();
    let peak_mem = p.memory_bytes.load(Ordering::Relaxed);
    let peak_cpu = p.cpu_percent.load(Ordering::Relaxed) as f64 / 100.0;

    let uptime_secs = {
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs()
    };

    serde_json::json!({
        "process": {
            "memory_mb": format!("{:.1}", proc_mem as f64 / 1024.0 / 1024.0),
            "memory_bytes": proc_mem,
            "cpu_percent": format!("{:.1}", proc_cpu),
            "uptime_seconds": uptime_secs,
            "uptime_human": format_uptime(uptime_secs),
        },
        "peak_24h": {
            "memory_mb": format!("{:.1}", peak_mem as f64 / 1024.0 / 1024.0),
            "memory_bytes": peak_mem,
            "cpu_percent": format!("{:.1}", peak_cpu),
        },
        "system": {
            "total_memory_mb": format!("{:.0}", sys.total_memory() as f64 / 1024.0 / 1024.0),
            "used_memory_mb": format!("{:.0}", sys.used_memory() as f64 / 1024.0 / 1024.0),
            "available_memory_mb": format!("{:.0}", sys.available_memory() as f64 / 1024.0 / 1024.0),
        },
    })
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

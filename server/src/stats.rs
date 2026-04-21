//! System stats tracking with 24h peak values

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
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
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs()
    };

    // Disk stats
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let root_disk = disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"));
    let (disk_total, disk_available) = root_disk
        .map(|d| (d.total_space(), d.available_space()))
        .unwrap_or((0, 0));

    serde_json::json!({
        "process": {
            "memory": format_bytes(proc_mem),
            "cpu": format!("{:.1}%", proc_cpu),
            "uptime": format_uptime(uptime_secs),
        },
        "peak_24h": {
            "memory": format_bytes(peak_mem),
            "cpu": format!("{:.1}%", peak_cpu),
        },
        "system": {
            "total_memory": format_bytes(sys.total_memory()),
            "used_memory": format_bytes(sys.used_memory()),
            "available_memory": format_bytes(sys.available_memory()),
        },
        "disk": {
            "total": format_bytes(disk_total),
            "available": format_bytes(disk_available),
            "used": format_bytes(disk_total.saturating_sub(disk_available)),
        },
    })
}

const DISK_WARNING_BYTES: u64 = 5 * 1_073_741_824; // 5 GB

pub fn disk_space_low() -> bool {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| d.available_space() < DISK_WARNING_BYTES)
        .unwrap_or(false)
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.0} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
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

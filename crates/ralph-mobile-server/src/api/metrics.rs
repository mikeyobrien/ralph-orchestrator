//! Host metrics endpoint.
//!
//! Provides real-time system metrics (CPU, memory, disk, network).

use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use sysinfo::{System, Networks, Disks};

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub usage_percent: f32,
    pub cores: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub usage_percent: f32,
    pub used_gb: f32,
    pub total_gb: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub usage_percent: f32,
    pub used_gb: f32,
    pub total_gb: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub download_mbps: f32,
    pub upload_mbps: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostMetrics {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub network: NetworkMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub name: String,
    pub cpu_percent: f32,
    pub memory_mb: f32,
    pub pid: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessList {
    pub processes: Vec<ProcessInfo>,
}

/// GET /api/metrics/host - Fetch current host metrics
pub async fn get_host_metrics() -> Result<HttpResponse> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU metrics
    let cpu_usage = sys.global_cpu_usage();
    let cpu_cores = sys.cpus().len();

    // Memory metrics
    let total_memory = sys.total_memory() as f32;
    let used_memory = sys.used_memory() as f32;
    let memory_percent = (used_memory / total_memory) * 100.0;

    // Disk metrics
    let disks = Disks::new_with_refreshed_list();
    let (total_disk, used_disk) = disks.iter().fold((0u64, 0u64), |(total, used), disk| {
        (
            total + disk.total_space(),
            used + (disk.total_space() - disk.available_space()),
        )
    });
    let total_disk_gb = total_disk as f32 / 1_073_741_824.0; // Convert bytes to GB
    let used_disk_gb = used_disk as f32 / 1_073_741_824.0;
    let disk_percent = if total_disk > 0 {
        (used_disk as f32 / total_disk as f32) * 100.0
    } else {
        0.0
    };

    // Network metrics
    let networks = Networks::new_with_refreshed_list();
    let (rx_bytes, tx_bytes) = networks.iter().fold((0u64, 0u64), |(rx, tx), (_, data)| {
        (rx + data.received(), tx + data.transmitted())
    });

    // Convert to Mbps (rough estimate, real rate would need time delta)
    let download_mbps = (rx_bytes as f32 / 1_048_576.0) / 60.0; // Assume 1-minute window
    let upload_mbps = (tx_bytes as f32 / 1_048_576.0) / 60.0;

    let metrics = HostMetrics {
        cpu: CpuMetrics {
            usage_percent: cpu_usage,
            cores: cpu_cores,
        },
        memory: MemoryMetrics {
            usage_percent: memory_percent,
            used_gb: used_memory / 1_073_741_824.0,
            total_gb: total_memory / 1_073_741_824.0,
        },
        disk: DiskMetrics {
            usage_percent: disk_percent,
            used_gb: used_disk_gb,
            total_gb: total_disk_gb,
        },
        network: NetworkMetrics {
            download_mbps,
            upload_mbps,
        },
    };

    Ok(HttpResponse::Ok().json(metrics))
}

/// GET /api/metrics/processes - Fetch top processes by CPU/memory
pub async fn get_processes() -> Result<HttpResponse> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, process)| ProcessInfo {
            name: process.name().to_string_lossy().to_string(),
            cpu_percent: process.cpu_usage(),
            memory_mb: process.memory() as f32 / 1_048_576.0,
            pid: pid.as_u32(),
        })
        .collect();

    // Sort by CPU usage descending, take top 20
    processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap());
    processes.truncate(20);

    Ok(HttpResponse::Ok().json(ProcessList { processes }))
}

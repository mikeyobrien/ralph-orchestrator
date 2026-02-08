//! Host metrics API for Ralph Mobile.
//!
//! Provides:
//! - GET /api/host/metrics - System metrics (CPU, memory, disk, network)
//! - GET /api/host/processes - Top processes by CPU usage

use actix_web::{HttpResponse, Responder};
use serde::Serialize;
use sysinfo::{Disks, Networks, System};

/// CPU metrics.
#[derive(Debug, Serialize)]
pub struct CpuMetrics {
    pub usage_percent: f32,
    pub cores: usize,
}

/// Memory metrics.
#[derive(Debug, Serialize)]
pub struct MemoryMetrics {
    pub usage_percent: f32,
    pub used_gb: f64,
    pub total_gb: f64,
}

/// Disk metrics.
#[derive(Debug, Serialize)]
pub struct DiskMetrics {
    pub usage_percent: f32,
    pub used_gb: f64,
    pub total_gb: f64,
}

/// Network metrics.
#[derive(Debug, Serialize)]
pub struct NetworkMetrics {
    pub download_mbps: f64,
    pub upload_mbps: f64,
}

/// Response for GET /api/host/metrics.
#[derive(Debug, Serialize)]
pub struct HostMetricsResponse {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub network: NetworkMetrics,
}

/// Process information.
#[derive(Debug, Serialize)]
pub struct ProcessInfo {
    pub name: String,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub pid: u32,
}

/// Response for GET /api/host/processes.
#[derive(Debug, Serialize)]
pub struct ProcessesResponse {
    pub processes: Vec<ProcessInfo>,
}

/// Handler for GET /api/host/metrics.
pub async fn get_metrics() -> impl Responder {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU metrics
    let cpu_usage = sys.global_cpu_usage();
    let cpu_cores = sys.cpus().len();

    // Memory metrics
    let total_memory = sys.total_memory() as f64;
    let used_memory = sys.used_memory() as f64;
    let memory_usage_percent = if total_memory > 0.0 {
        (used_memory / total_memory * 100.0) as f32
    } else {
        0.0
    };

    // Disk metrics (aggregate all disks)
    let disks = Disks::new_with_refreshed_list();
    let (total_disk, used_disk) = disks.iter().fold((0u64, 0u64), |(total, used), disk| {
        let disk_total = disk.total_space();
        let disk_available = disk.available_space();
        (total + disk_total, used + (disk_total - disk_available))
    });
    let disk_usage_percent = if total_disk > 0 {
        (used_disk as f64 / total_disk as f64 * 100.0) as f32
    } else {
        0.0
    };

    // Network metrics (aggregate all interfaces)
    let networks = Networks::new_with_refreshed_list();
    let (total_received, total_transmitted) =
        networks
            .iter()
            .fold((0u64, 0u64), |(rx, tx), (_name, data)| {
                (rx + data.received(), tx + data.transmitted())
            });

    // Convert bytes to Mbps (rough estimate based on refresh interval)
    // Since this is a point-in-time snapshot, we report cumulative as a proxy
    let download_mbps = (total_received as f64) / 1_000_000.0;
    let upload_mbps = (total_transmitted as f64) / 1_000_000.0;

    let response = HostMetricsResponse {
        cpu: CpuMetrics {
            usage_percent: cpu_usage,
            cores: cpu_cores,
        },
        memory: MemoryMetrics {
            usage_percent: memory_usage_percent,
            used_gb: used_memory / 1_073_741_824.0, // bytes to GB
            total_gb: total_memory / 1_073_741_824.0,
        },
        disk: DiskMetrics {
            usage_percent: disk_usage_percent,
            used_gb: used_disk as f64 / 1_073_741_824.0,
            total_gb: total_disk as f64 / 1_073_741_824.0,
        },
        network: NetworkMetrics {
            download_mbps,
            upload_mbps,
        },
    };

    HttpResponse::Ok().json(response)
}

/// Handler for GET /api/host/processes.
pub async fn get_processes() -> impl Responder {
    let mut sys = System::new_all();
    sys.refresh_all();

    // Collect all processes with their CPU usage
    let mut processes: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .map(|(pid, process)| ProcessInfo {
            name: process.name().to_string_lossy().to_string(),
            cpu_percent: process.cpu_usage(),
            memory_mb: process.memory() as f64 / 1_048_576.0, // bytes to MB
            pid: pid.as_u32(),
        })
        .collect();

    // Sort by CPU usage (descending) and take top 10
    processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
    processes.truncate(10);

    HttpResponse::Ok().json(ProcessesResponse { processes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{web, App};

    #[test]
    fn test_cpu_metrics_serialization() {
        let cpu = CpuMetrics {
            usage_percent: 45.5, // Use value exactly representable in f32
            cores: 8,
        };
        let json = serde_json::to_value(&cpu).unwrap();
        assert_eq!(json["usage_percent"], 45.5);
        assert_eq!(json["cores"], 8);
    }

    #[test]
    fn test_memory_metrics_serialization() {
        let memory = MemoryMetrics {
            usage_percent: 62.5, // Use value exactly representable in f32
            used_gb: 12.5,
            total_gb: 32.0,
        };
        let json = serde_json::to_value(&memory).unwrap();
        assert_eq!(json["usage_percent"], 62.5);
        assert_eq!(json["used_gb"], 12.5);
        assert_eq!(json["total_gb"], 32.0);
    }

    #[test]
    fn test_disk_metrics_serialization() {
        let disk = DiskMetrics {
            usage_percent: 55.0,
            used_gb: 250.0,
            total_gb: 500.0,
        };
        let json = serde_json::to_value(&disk).unwrap();
        assert_eq!(json["usage_percent"], 55.0);
        assert_eq!(json["used_gb"], 250.0);
        assert_eq!(json["total_gb"], 500.0);
    }

    #[test]
    fn test_network_metrics_serialization() {
        let network = NetworkMetrics {
            download_mbps: 12.5,
            upload_mbps: 3.2,
        };
        let json = serde_json::to_value(&network).unwrap();
        assert_eq!(json["download_mbps"], 12.5);
        assert_eq!(json["upload_mbps"], 3.2);
    }

    #[test]
    fn test_process_info_serialization() {
        let process = ProcessInfo {
            name: "ralph".to_string(),
            cpu_percent: 25.0,
            memory_mb: 512.0,
            pid: 1234,
        };
        let json = serde_json::to_value(&process).unwrap();
        assert_eq!(json["name"], "ralph");
        assert_eq!(json["cpu_percent"], 25.0);
        assert_eq!(json["memory_mb"], 512.0);
        assert_eq!(json["pid"], 1234);
    }

    #[test]
    fn test_host_metrics_response_serialization() {
        let response = HostMetricsResponse {
            cpu: CpuMetrics {
                usage_percent: 45.2,
                cores: 8,
            },
            memory: MemoryMetrics {
                usage_percent: 62.1,
                used_gb: 12.5,
                total_gb: 32.0,
            },
            disk: DiskMetrics {
                usage_percent: 55.0,
                used_gb: 250.0,
                total_gb: 500.0,
            },
            network: NetworkMetrics {
                download_mbps: 12.5,
                upload_mbps: 3.2,
            },
        };
        let json = serde_json::to_value(&response).unwrap();
        assert!(json["cpu"].is_object());
        assert!(json["memory"].is_object());
        assert!(json["disk"].is_object());
        assert!(json["network"].is_object());
    }

    #[test]
    fn test_processes_response_serialization() {
        let response = ProcessesResponse {
            processes: vec![
                ProcessInfo {
                    name: "ralph".to_string(),
                    cpu_percent: 25.0,
                    memory_mb: 512.0,
                    pid: 1234,
                },
                ProcessInfo {
                    name: "cargo".to_string(),
                    cpu_percent: 15.0,
                    memory_mb: 256.0,
                    pid: 5678,
                },
            ],
        };
        let json = serde_json::to_value(&response).unwrap();
        assert!(json["processes"].is_array());
        assert_eq!(json["processes"].as_array().unwrap().len(), 2);
    }

    // Functional API tests

    #[actix_web::test]
    async fn test_get_metrics_returns_json() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/host/metrics", web::get().to(get_metrics)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/host/metrics")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[actix_web::test]
    async fn test_get_metrics_has_required_sections() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/host/metrics", web::get().to(get_metrics)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/host/metrics")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json.get("cpu").is_some());
        assert!(json.get("memory").is_some());
        assert!(json.get("disk").is_some());
        assert!(json.get("network").is_some());
    }

    #[actix_web::test]
    async fn test_get_processes_returns_json() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/host/processes", web::get().to(get_processes)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/host/processes")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").unwrap();
        assert!(content_type.to_str().unwrap().contains("application/json"));
    }

    #[actix_web::test]
    async fn test_get_processes_has_processes_array() {
        let app = actix_web::test::init_service(
            App::new().service(
                web::scope("/api").route("/host/processes", web::get().to(get_processes)),
            ),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/api/host/processes")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        let body = actix_web::test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["processes"].is_array());
    }
}

//! Parallel Loops API endpoints.
//!
//! GET /api/loops - List all loops from registry
//! POST /api/loops - Spawn a new worktree loop
//! GET /api/loops/{id} - Get specific loop details
//! POST /api/loops/{id}/merge - Merge a worktree loop
//! POST /api/loops/{id}/discard - Discard a worktree loop

use actix_web::{web, HttpResponse, Responder};
use ralph_core::loop_registry::{LoopEntry, LoopRegistry};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::sessions::ErrorResponse;

/// Response format for loop list items.
#[derive(Debug, Serialize)]
pub struct LoopInfo {
    pub id: String,
    pub status: String, // "primary" | "worktree"
    pub prompt: String,
    pub pid: u32,
    pub started_at: String, // ISO 8601 format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    pub workspace: String,
}

impl From<&LoopEntry> for LoopInfo {
    fn from(entry: &LoopEntry) -> Self {
        let status = if entry.worktree_path.is_some() {
            "worktree"
        } else {
            "primary"
        };

        LoopInfo {
            id: entry.id.clone(),
            status: status.to_string(),
            prompt: entry.prompt.clone(),
            pid: entry.pid,
            started_at: entry.started.to_rfc3339(),
            worktree_path: entry.worktree_path.clone(),
            workspace: entry.workspace.clone(),
        }
    }
}

/// Response format for loop list.
#[derive(Debug, Serialize)]
pub struct LoopsResponse {
    pub loops: Vec<LoopInfo>,
}

/// Success response for merge/discard operations.
#[derive(Debug, Serialize)]
pub struct OperationResponse {
    pub success: bool,
    pub message: String,
}

/// Request body for POST /api/loops.
#[derive(Debug, Deserialize)]
pub struct SpawnLoopRequest {
    pub prompt: String,
    #[serde(default)]
    pub config_path: Option<String>,
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
}

fn default_base_branch() -> String {
    "main".to_string()
}

/// Response body for POST /api/loops.
#[derive(Debug, Serialize)]
pub struct SpawnLoopResponse {
    pub id: String,
    pub worktree_path: String,
    pub status: String,
}

/// POST /api/loops - Spawn a new worktree loop.
///
/// This operation:
/// 1. Validates the workspace and config (if provided)
/// 2. Executes `ralph run --worktree -p "prompt"` command
/// 3. Returns the worktree path and loop ID
pub async fn spawn_loop(body: web::Json<SpawnLoopRequest>) -> impl Responder {
    let workspace = match find_workspace_root() {
        Some(root) => root,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "workspace_not_found: .ralph directory not found".to_string(),
            });
        }
    };

    // Validate config exists if provided
    if let Some(ref config_path) = body.config_path {
        let full_config_path = workspace.join(config_path);
        if !full_config_path.exists() {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: format!("config_not_found: {}", config_path),
            });
        }
    }

    // Build ralph command
    let mut cmd = Command::new("ralph");
    cmd.current_dir(&workspace)
        .arg("run")
        .arg("--worktree")
        .arg("--base-branch")
        .arg(&body.base_branch)
        .arg("-p")
        .arg(&body.prompt);

    if let Some(ref config_path) = body.config_path {
        cmd.arg("--config").arg(config_path);
    }

    // Spawn the process in background
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match cmd.spawn() {
        Ok(child) => {
            let pid = child.id();

            // Wait briefly to ensure worktree is created
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Try to find the worktree path by looking at the registry
            // The loop should have registered itself
            let registry = LoopRegistry::new(&workspace);

            // Find the loop entry for this PID
            let loop_entry = registry.list()
                .ok()
                .and_then(|entries| {
                    entries.into_iter()
                        .find(|e| e.pid == pid && e.worktree_path.is_some())
                });

            match loop_entry {
                Some(entry) => {
                    HttpResponse::Created().json(SpawnLoopResponse {
                        id: entry.id,
                        worktree_path: entry.worktree_path.unwrap_or_default(),
                        status: "starting".to_string(),
                    })
                }
                None => {
                    // Fallback: generate expected worktree path
                    let worktree_path = workspace
                        .join(".worktrees")
                        .join(format!("worktree-{}", pid));

                    HttpResponse::Created().json(SpawnLoopResponse {
                        id: format!("worktree-{}", pid),
                        worktree_path: worktree_path.display().to_string(),
                        status: "starting".to_string(),
                    })
                }
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("failed_to_spawn: {}", e),
        }),
    }
}

/// GET /api/loops - List all loops from registry.
pub async fn list_loops() -> impl Responder {
    // Determine workspace root - look for .ralph directory
    let workspace = match find_workspace_root() {
        Some(root) => root,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "workspace_not_found: .ralph directory not found".to_string(),
            });
        }
    };

    let registry = LoopRegistry::new(&workspace);

    match registry.list() {
        Ok(entries) => {
            let loops: Vec<LoopInfo> = entries.iter().map(LoopInfo::from).collect();
            HttpResponse::Ok().json(LoopsResponse { loops })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("registry_error: {}", e),
        }),
    }
}

/// GET /api/loops/{id} - Get specific loop details.
pub async fn get_loop(path: web::Path<String>) -> impl Responder {
    let loop_id = path.into_inner();

    let workspace = match find_workspace_root() {
        Some(root) => root,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "workspace_not_found: .ralph directory not found".to_string(),
            });
        }
    };

    let registry = LoopRegistry::new(&workspace);

    match registry.get(&loop_id) {
        Ok(Some(entry)) => HttpResponse::Ok().json(LoopInfo::from(&entry)),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("loop_not_found: {}", loop_id),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("registry_error: {}", e),
        }),
    }
}

/// POST /api/loops/{id}/merge - Merge a worktree loop.
///
/// This operation:
/// 1. Validates the loop exists and is a worktree loop
/// 2. Executes `ralph loops merge <id>` command
/// 3. Returns success/failure status
pub async fn merge_loop(path: web::Path<String>) -> impl Responder {
    let loop_id = path.into_inner();

    let workspace = match find_workspace_root() {
        Some(root) => root,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "workspace_not_found: .ralph directory not found".to_string(),
            });
        }
    };

    let registry = LoopRegistry::new(&workspace);

    // Validate loop exists and is a worktree loop
    match registry.get(&loop_id) {
        Ok(Some(entry)) => {
            if entry.worktree_path.is_none() {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    error: "invalid_operation: cannot merge primary loop".to_string(),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: format!("loop_not_found: {}", loop_id),
            });
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("registry_error: {}", e),
            });
        }
    }

    // Execute merge command
    match execute_ralph_command(&workspace, &["loops", "merge", &loop_id]) {
        Ok(output) => {
            if output.status.success() {
                HttpResponse::Ok().json(OperationResponse {
                    success: true,
                    message: format!("Loop {} merged successfully", loop_id),
                })
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                HttpResponse::InternalServerError().json(ErrorResponse {
                    error: format!("merge_failed: {}", stderr),
                })
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("command_error: {}", e),
        }),
    }
}

/// POST /api/loops/{id}/discard - Discard a worktree loop.
///
/// This operation:
/// 1. Validates the loop exists and is a worktree loop
/// 2. Executes `ralph loops discard <id>` command
/// 3. Returns success/failure status
pub async fn discard_loop(path: web::Path<String>) -> impl Responder {
    let loop_id = path.into_inner();

    let workspace = match find_workspace_root() {
        Some(root) => root,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "workspace_not_found: .ralph directory not found".to_string(),
            });
        }
    };

    let registry = LoopRegistry::new(&workspace);

    // Validate loop exists and is a worktree loop
    match registry.get(&loop_id) {
        Ok(Some(entry)) => {
            if entry.worktree_path.is_none() {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    error: "invalid_operation: cannot discard primary loop".to_string(),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: format!("loop_not_found: {}", loop_id),
            });
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("registry_error: {}", e),
            });
        }
    }

    // Execute discard command
    match execute_ralph_command(&workspace, &["loops", "discard", &loop_id]) {
        Ok(output) => {
            if output.status.success() {
                HttpResponse::Ok().json(OperationResponse {
                    success: true,
                    message: format!("Loop {} discarded successfully", loop_id),
                })
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                HttpResponse::InternalServerError().json(ErrorResponse {
                    error: format!("discard_failed: {}", stderr),
                })
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: format!("command_error: {}", e),
        }),
    }
}

/// Find the workspace root by walking up from current directory until .ralph is found.
fn find_workspace_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        let ralph_dir = current.join(".ralph");
        if ralph_dir.exists() && ralph_dir.is_dir() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

/// Execute a ralph command in the workspace.
fn execute_ralph_command(
    workspace: &PathBuf,
    args: &[&str],
) -> std::io::Result<std::process::Output> {
    std::process::Command::new("ralph")
        .args(args)
        .current_dir(workspace)
        .output()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_registry(workspace: &std::path::Path) -> LoopRegistry {
        // Create .ralph directory
        fs::create_dir_all(workspace.join(".ralph")).unwrap();
        LoopRegistry::new(workspace)
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_list_loops_empty() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = create_test_registry(temp_dir.path());

        // Register no loops

        // Change to temp dir so find_workspace_root works
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir.clone(), |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/loops", web::get().to(list_loops)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/loops")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(json["loops"].is_array());
        assert_eq!(json["loops"].as_array().unwrap().len(), 0);
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_list_loops_with_entries() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry(temp_dir.path());

        // Register a primary loop
        let entry1 = LoopEntry::with_workspace(
            "test prompt 1",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        let id1 = entry1.id.clone();
        registry.register(entry1).unwrap();

        // Register a worktree loop with a different PID (use parent PID which is also alive)
        let mut entry2 = LoopEntry::with_workspace(
            "test prompt 2",
            Some("/path/to/worktree"),
            temp_dir.path().display().to_string(),
        );
        // Use parent PID to avoid duplicate PID removal in registry
        entry2.pid = nix::unistd::getppid().as_raw() as u32;
        registry.register(entry2).unwrap();


        // Change to temp dir
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir.clone(), |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/loops", web::get().to(list_loops)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/loops")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let loops = json["loops"].as_array().unwrap();
        assert_eq!(loops.len(), 2);

        // Check first loop (primary)
        let loop1 = loops.iter().find(|l| l["id"] == id1).unwrap();
        assert_eq!(loop1["status"], "primary");
        assert_eq!(loop1["prompt"], "test prompt 1");
        assert!(loop1["worktree_path"].is_null());

        // Check second loop (worktree)
        let loop2 = loops.iter().find(|l| l["status"] == "worktree").unwrap();
        assert_eq!(loop2["prompt"], "test prompt 2");
        assert_eq!(loop2["worktree_path"], "/path/to/worktree");
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_get_loop_success() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry(temp_dir.path());

        let entry = LoopEntry::with_workspace(
            "test prompt",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        let id = entry.id.clone();
        registry.register(entry).unwrap();


        // Change to temp dir
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir.clone(), |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/loops/{id}", web::get().to(get_loop)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/api/loops/{}", id))
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["id"], id);
        assert_eq!(json["status"], "primary");
        assert_eq!(json["prompt"], "test prompt");
    }

    #[actix_web::test]
    #[serial(cwd)]
    async fn test_get_loop_not_found() {
        let temp_dir = TempDir::new().unwrap();
        create_test_registry(temp_dir.path());


        // Change to temp dir
        let original_dir = std::env::current_dir().unwrap();
        let _guard = scopeguard::guard(original_dir, |dir| {
            let _ = std::env::set_current_dir(dir);
        });
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let app = test::init_service(
            App::new().service(
                web::scope("/api")
                    .route("/loops/{id}", web::get().to(get_loop)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/api/loops/nonexistent")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);

        let body = test::read_body(resp).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("loop_not_found"));
    }

    #[actix_web::test]
    async fn test_loop_info_from_entry() {
        let entry = LoopEntry::with_workspace(
            "test prompt",
            Some("/worktree"),
            "/workspace",
        );

        let info = LoopInfo::from(&entry);

        assert_eq!(info.id, entry.id);
        assert_eq!(info.status, "worktree");
        assert_eq!(info.prompt, "test prompt");
        assert_eq!(info.pid, entry.pid);
        assert_eq!(info.worktree_path, Some("/worktree".to_string()));
        assert_eq!(info.workspace, "/workspace");
    }

    #[actix_web::test]
    async fn test_loop_info_primary_status() {
        let entry = LoopEntry::with_workspace(
            "test prompt",
            None::<String>,
            "/workspace",
        );

        let info = LoopInfo::from(&entry);

        assert_eq!(info.status, "primary");
        assert!(info.worktree_path.is_none());
    }
}

// ABOUTME: Web dashboard development server launcher.
// ABOUTME: Provides the `ralph web` command that runs backend and frontend dev servers in parallel.

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command as AsyncCommand};
use tokio::sync::Notify;

#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;

/// Grace period for servers to shut down before SIGKILL (matches backend's SIGINT handler)
const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(10);

/// Timeout for both servers to become ready
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Arguments for the web subcommand
#[derive(Parser, Debug)]
pub struct WebArgs {
    /// Backend port (default: 3000)
    #[arg(long, default_value = "3000")]
    pub backend_port: u16,

    /// Frontend port (default: 5173)
    #[arg(long, default_value = "5173")]
    pub frontend_port: u16,

    /// Workspace root directory (default: current directory)
    #[arg(long)]
    pub workspace: Option<PathBuf>,

    /// Don't open the dashboard in the default browser
    #[arg(long)]
    pub no_open: bool,
}

/// Check that Node.js is installed and >= 18. Returns the version string.
fn check_node_with(node_cmd: &OsStr) -> Result<String> {
    let output = Command::new(node_cmd)
        .arg("--version")
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "Node.js is not installed or not in PATH.\n\
                 Install Node.js 18+: https://nodejs.org/\n\
                 Or via nvm: nvm install 18"
            )
        })?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to run `node --version`.\n\
             Install Node.js 18+: https://nodejs.org/"
        );
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Parse major version from e.g. "v18.17.0"
    let major: u32 = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if major < 18 {
        anyhow::bail!(
            "Node.js {} is too old (need >= 18).\n\
             Update: https://nodejs.org/ or `nvm install 18`",
            version
        );
    }

    Ok(version)
}

/// Check that npm is installed and working. Returns the version string.
fn check_npm_with(npm_cmd: &OsStr) -> Result<String> {
    let output = Command::new(npm_cmd)
        .arg("--version")
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "npm is not installed or not in PATH.\n\
             npm should come with Node.js. Try reinstalling Node: https://nodejs.org/"
            )
        })?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to run `npm --version`.\n\
             Try reinstalling Node.js: https://nodejs.org/"
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if npm dependencies need to be installed.
fn needs_install(root: &Path) -> bool {
    !root.join("node_modules/.package-lock.json").exists()
}

/// Run npm install (or npm ci if lockfile present) with a spinner.
async fn run_npm_install_with(root: &Path, npm_cmd: &OsStr) -> Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid template"),
    );

    let has_lockfile = root.join("package-lock.json").exists();
    let install_cmd = if has_lockfile { "ci" } else { "install" };

    spinner.set_message(format!("Running npm {}...", install_cmd));
    spinner.enable_steady_tick(Duration::from_millis(100));

    let output = AsyncCommand::new(npm_cmd)
        .arg(install_cmd)
        .current_dir(root)
        .output()
        .await
        .context("Failed to run npm install")?;

    spinner.finish_and_clear();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("npm {} failed:\n{}", install_cmd, stderr.trim());
    }

    println!("Dependencies installed successfully.");
    Ok(())
}

/// Check that a TCP port is available for binding.
fn check_port_available(port: u16) -> Result<()> {
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => Ok(()),
        Err(_) => {
            anyhow::bail!(
                "Port {} is already in use.\n\
                 Use --backend-port or --frontend-port to pick a different port.\n\
                 To free the port: fuser -k {}/tcp",
                port,
                port
            );
        }
    }
}

/// Check for tsx 4.20.0 which has known issues.
fn check_tsx_version_with(backend_dir: &Path, npx_cmd: &OsStr) -> Result<()> {
    let output = Command::new(npx_cmd)
        .args(["tsx", "--version"])
        .current_dir(backend_dir)
        .output();

    if let Ok(output) = output {
        let version = String::from_utf8_lossy(&output.stdout);
        let token = version.split_whitespace().last().unwrap_or("");
        let cleaned = token.trim_start_matches('v');
        if cleaned == "4.20.0" {
            anyhow::bail!(
                "tsx 4.20.0 has known issues that affect the web server.\n\
                 Fix: npm install tsx@^4.21.0 -w @ralph-web/server"
            );
        }
    }
    // If we can't run tsx or it doesn't match, proceed silently
    Ok(())
}

/// Run pre-flight checks: verify Node.js/npm, check tsx, and auto-install dependencies.
async fn preflight_with(
    root: &Path,
    backend_dir: &Path,
    node_cmd: &OsStr,
    npm_cmd: &OsStr,
    npx_cmd: &OsStr,
) -> Result<()> {
    let node_version = check_node_with(node_cmd)?;
    let npm_version = check_npm_with(npm_cmd)?;
    println!(
        "Using Node {} with npm {}",
        node_version.trim_start_matches('v'),
        npm_version
    );

    if needs_install(root) {
        println!("node_modules not found — installing dependencies...");
        run_npm_install_with(root, npm_cmd).await?;
    }

    check_tsx_version_with(backend_dir, npx_cmd)?;

    Ok(())
}

async fn preflight(root: &Path, backend_dir: &Path) -> Result<()> {
    preflight_with(
        root,
        backend_dir,
        OsStr::new("node"),
        OsStr::new("npm"),
        OsStr::new("npx"),
    )
    .await
}

/// Forward output from a child process, prefixing each line with a label.
/// Notifies `ready` when the output contains the given ready pattern.
async fn forward_output(
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    label: &str,
    ready_pattern: &str,
    ready: std::sync::Arc<Notify>,
) {
    let label_out = label.to_string();
    let label_err = label.to_string();
    let pattern_out = ready_pattern.to_string();
    let pattern_err = ready_pattern.to_string();
    let ready_out = ready.clone();
    let ready_err = ready;

    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut notified = false;
        while let Ok(Some(line)) = lines.next_line().await {
            println!("[{}] {}", label_out, line);
            if !notified && line.contains(&pattern_out) {
                ready_out.notify_one();
                notified = true;
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut notified = false;
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("[{}] {}", label_err, line);
            if !notified && line.contains(&pattern_err) {
                ready_err.notify_one();
                notified = true;
            }
        }
    });

    // Run both forwarding tasks to completion (they end when the process closes its pipes)
    let _ = tokio::join!(stdout_task, stderr_task);
}

/// Run both backend and frontend dev servers in parallel
pub async fn execute(args: WebArgs) -> Result<()> {
    println!("Starting Ralph web servers...");

    // Determine workspace root: explicit flag or current directory
    let workspace_root = match args.workspace {
        Some(path) => {
            // Canonicalize to get absolute path
            path.canonicalize()
                .with_context(|| format!("Invalid workspace path: {}", path.display()))?
        }
        None => env::current_dir().context("Failed to get current directory")?,
    };

    // Compute absolute paths for backend and frontend directories
    let backend_dir = workspace_root.join("backend/ralph-web-server");
    let frontend_dir = workspace_root.join("frontend/ralph-web");

    // Verify Node.js/npm, check tsx version, and auto-install dependencies if needed
    preflight(&workspace_root, &backend_dir).await?;

    // Check ports before spawning anything
    check_port_available(args.backend_port)?;
    check_port_available(args.frontend_port)?;

    println!("Using workspace: {}", workspace_root.display());

    // Spawn backend server with piped output
    // Pass RALPH_WORKSPACE_ROOT so the backend knows where to spawn ralph run from
    // Pass PORT so the backend listens on the configured port
    let mut backend = AsyncCommand::new("npm")
        .args(["run", "dev"])
        .current_dir(&backend_dir)
        .env("RALPH_WORKSPACE_ROOT", &workspace_root)
        .env("PORT", args.backend_port.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to start backend server. Is npm installed and {} set up?\nError: {}",
                backend_dir.join("package.json").display(),
                e
            )
        })?;

    // Spawn frontend server with piped output
    // Pass --port for Vite and RALPH_BACKEND_PORT for proxy config
    let mut frontend = AsyncCommand::new("npm")
        .args([
            "run",
            "dev",
            "--",
            "--port",
            &args.frontend_port.to_string(),
        ])
        .current_dir(&frontend_dir)
        .env("RALPH_BACKEND_PORT", args.backend_port.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to start frontend server. Is npm installed and {} set up?\nError: {}",
                frontend_dir.join("package.json").display(),
                e
            )
        })?;

    // Take ownership of stdout/stderr pipes
    let backend_stdout = backend.stdout.take().expect("backend stdout piped");
    let backend_stderr = backend.stderr.take().expect("backend stderr piped");
    let frontend_stdout = frontend.stdout.take().expect("frontend stdout piped");
    let frontend_stderr = frontend.stderr.take().expect("frontend stderr piped");

    // Set up ready detection
    let backend_ready = std::sync::Arc::new(Notify::new());
    let frontend_ready = std::sync::Arc::new(Notify::new());

    // Spawn output forwarding tasks
    let backend_ready_clone = backend_ready.clone();
    tokio::spawn(async move {
        forward_output(
            backend_stdout,
            backend_stderr,
            "backend",
            "Server started on",
            backend_ready_clone,
        )
        .await;
    });

    let frontend_ready_clone = frontend_ready.clone();
    tokio::spawn(async move {
        forward_output(
            frontend_stdout,
            frontend_stderr,
            "frontend",
            "Local:",
            frontend_ready_clone,
        )
        .await;
    });

    // Wait for both servers to become ready
    let dashboard_url = format!("http://localhost:{}", args.frontend_port);
    let api_url = format!("http://localhost:{}", args.backend_port);

    let ready_result = tokio::time::timeout(READY_TIMEOUT, async {
        tokio::join!(backend_ready.notified(), frontend_ready.notified());
    })
    .await;

    match ready_result {
        Ok(()) => {
            println!();
            println!("Both servers ready!");
            println!("  Dashboard: {}", dashboard_url);
            println!("  API:       {}", api_url);
            println!();

            if !args.no_open {
                let _ = open::that(&dashboard_url);
            }
        }
        Err(_) => {
            println!();
            println!(
                "Warning: servers did not report ready within {} seconds.",
                READY_TIMEOUT.as_secs()
            );
            println!("They may still be starting. Check the output above for errors.");
            println!("  Dashboard: {}", dashboard_url);
            println!("  API:       {}", api_url);
            println!();
        }
    }

    println!("Press Ctrl+C to stop both servers");

    // Set up shutdown channel
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    // Spawn signal handlers
    let shutdown_tx_sigint = shutdown_tx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            println!("\nReceived Ctrl+C, shutting down servers...");
            let _ = shutdown_tx_sigint.send(true);
        }
    });

    #[cfg(unix)]
    {
        let shutdown_tx_sigterm = shutdown_tx.clone();
        tokio::spawn(async move {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to register SIGTERM handler");
            sigterm.recv().await;
            println!("\nReceived SIGTERM, shutting down servers...");
            let _ = shutdown_tx_sigterm.send(true);
        });

        let shutdown_tx_sighup = shutdown_tx.clone();
        tokio::spawn(async move {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("Failed to register SIGHUP handler");
            sighup.recv().await;
            println!("\nReceived SIGHUP (terminal closed), shutting down servers...");
            let _ = shutdown_tx_sighup.send(true);
        });
    }

    // Wait for shutdown signal or server exit
    tokio::select! {
        _ = shutdown_rx.changed() => {
            // Signal received - gracefully terminate both servers
            println!("Stopping backend server...");
            terminate_gracefully(&mut backend, SHUTDOWN_GRACE_PERIOD).await;
            println!("Stopping frontend server...");
            terminate_gracefully(&mut frontend, SHUTDOWN_GRACE_PERIOD).await;
            println!("All servers stopped.");
        }
        r = backend.wait() => {
            println!("Backend exited: {:?}", r);
            // Gracefully terminate frontend on backend exit
            println!("Stopping frontend server...");
            terminate_gracefully(&mut frontend, SHUTDOWN_GRACE_PERIOD).await;
        }
        r = frontend.wait() => {
            println!("Frontend exited: {:?}", r);
            // Gracefully terminate backend on frontend exit
            println!("Stopping backend server...");
            terminate_gracefully(&mut backend, SHUTDOWN_GRACE_PERIOD).await;
        }
    }

    Ok(())
}

/// Gracefully terminate a child process by sending SIGTERM first, then SIGKILL after grace period
#[cfg(unix)]
async fn terminate_gracefully(child: &mut Child, grace_period: Duration) {
    if let Some(pid) = child.id() {
        let pid = Pid::from_raw(pid as i32);

        // Send SIGTERM for graceful shutdown
        if kill(pid, Signal::SIGTERM).is_err() {
            // Process may have already exited
            let _ = child.wait().await;
            return;
        }

        // Wait for graceful exit with timeout
        match tokio::time::timeout(grace_period, child.wait()).await {
            Ok(_) => {
                // Process exited gracefully
            }
            Err(_) => {
                // Grace period elapsed, force kill
                println!("  Grace period elapsed, forcing termination...");
                let _ = kill(pid, Signal::SIGKILL);
                let _ = child.wait().await;
            }
        }
    } else {
        // No PID means process already exited or wasn't started
        let _ = child.wait().await;
    }
}

/// Gracefully terminate a child process (non-Unix fallback using start_kill)
#[cfg(not(unix))]
async fn terminate_gracefully(child: &mut Child, _grace_period: Duration) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[cfg(unix)]
    fn write_fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        let script = format!("#!/bin/sh\n{}\n", body);
        std::fs::write(&path, script).expect("write script");
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    #[test]
    fn needs_install_detects_missing_node_modules() {
        let temp_dir = TempDir::new().expect("temp dir");
        let root = temp_dir.path();

        assert!(needs_install(root));

        let node_modules = root.join("node_modules");
        std::fs::create_dir_all(&node_modules).expect("create node_modules");
        std::fs::write(node_modules.join(".package-lock.json"), "").expect("write lock");

        assert!(!needs_install(root));
    }

    #[test]
    fn check_port_available_detects_in_use() {
        match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                let port = listener.local_addr().expect("addr").port();
                assert!(check_port_available(port).is_err());
                drop(listener);
                assert!(check_port_available(port).is_ok());
            }
            Err(err) => {
                // Some sandboxes disallow binding; ensure we handle that path gracefully.
                assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(check_port_available(0).is_err());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn check_node_accepts_supported_version() {
        let temp_dir = TempDir::new().expect("temp dir");
        let node_path = write_fake_executable(temp_dir.path(), "node", "echo v18.17.0");
        let version = check_node_with(node_path.as_os_str()).expect("check node");
        assert_eq!(version.trim(), "v18.17.0");
    }

    #[cfg(unix)]
    #[test]
    fn check_node_rejects_old_version() {
        let temp_dir = TempDir::new().expect("temp dir");
        let node_path = write_fake_executable(temp_dir.path(), "node", "echo v16.5.0");
        let err = check_node_with(node_path.as_os_str()).expect_err("expected version error");
        let msg = format!("{err}");
        assert!(msg.contains("too old"), "msg: {msg}");
    }

    #[test]
    fn check_node_reports_missing_binary() {
        let err =
            check_node_with(OsStr::new("definitely-missing-node-12345")).expect_err("missing");
        let msg = format!("{err}");
        assert!(msg.contains("Node.js is not installed"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_node_reports_failed_command() {
        let temp_dir = TempDir::new().expect("temp dir");
        let node_path = write_fake_executable(temp_dir.path(), "node", "exit 1");
        let err = check_node_with(node_path.as_os_str()).expect_err("node failure");
        let msg = format!("{err}");
        assert!(msg.contains("Failed to run `node --version`"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_npm_reads_version() {
        let temp_dir = TempDir::new().expect("temp dir");
        let npm_path = write_fake_executable(temp_dir.path(), "npm", "echo 9.1.0");
        let version = check_npm_with(npm_path.as_os_str()).expect("check npm");
        assert_eq!(version.trim(), "9.1.0");
    }

    #[cfg(unix)]
    #[test]
    fn check_npm_reports_failed_command() {
        let temp_dir = TempDir::new().expect("temp dir");
        let npm_path = write_fake_executable(temp_dir.path(), "npm", "exit 1");
        let err = check_npm_with(npm_path.as_os_str()).expect_err("npm failure");
        let msg = format!("{err}");
        assert!(msg.contains("Failed to run `npm --version`"), "msg: {msg}");
    }

    #[test]
    fn check_npm_reports_missing_binary() {
        let err = check_npm_with(OsStr::new("definitely-missing-npm-12345")).expect_err("missing");
        let msg = format!("{err}");
        assert!(msg.contains("npm is not installed"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_tsx_version_blocks_known_bad_release() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend_dir = temp_dir.path().join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");
        let npx_path = write_fake_executable(temp_dir.path(), "npx", "echo 4.20.0");
        let err =
            check_tsx_version_with(&backend_dir, npx_path.as_os_str()).expect_err("tsx error");
        let msg = format!("{err}");
        assert!(msg.contains("tsx 4.20.0"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_tsx_version_blocks_known_bad_release_with_v_prefix() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend_dir = temp_dir.path().join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");
        let npx_path = write_fake_executable(temp_dir.path(), "npx", "echo v4.20.0");
        let err =
            check_tsx_version_with(&backend_dir, npx_path.as_os_str()).expect_err("tsx error");
        let msg = format!("{err}");
        assert!(msg.contains("tsx 4.20.0"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_tsx_version_blocks_known_bad_release_with_label() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend_dir = temp_dir.path().join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");
        let npx_path = write_fake_executable(temp_dir.path(), "npx", "echo tsx v4.20.0");
        let err =
            check_tsx_version_with(&backend_dir, npx_path.as_os_str()).expect_err("tsx error");
        let msg = format!("{err}");
        assert!(msg.contains("tsx 4.20.0"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preflight_runs_install_with_fake_tools() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let node_path = write_fake_executable(&bin_dir, "node", "echo v20.1.0");
        let npx_path = write_fake_executable(&bin_dir, "npx", "echo 4.21.0");
        let npm_path = write_fake_executable(
            &bin_dir,
            "npm",
            "if [ \"$1\" = \"--version\" ]; then echo 9.6.0; else touch npm_install_called; fi",
        );

        let root = temp_dir.path().join("workspace");
        let backend_dir = root.join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");

        preflight_with(
            &root,
            &backend_dir,
            node_path.as_os_str(),
            npm_path.as_os_str(),
            npx_path.as_os_str(),
        )
        .await
        .expect("preflight");
        assert!(root.join("npm_install_called").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preflight_skips_install_when_node_modules_present() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let node_path = write_fake_executable(&bin_dir, "node", "echo v20.1.0");
        let npx_path = write_fake_executable(&bin_dir, "npx", "echo 4.21.0");
        let npm_path = write_fake_executable(
            &bin_dir,
            "npm",
            "if [ \"$1\" = \"--version\" ]; then echo 9.6.0; exit 0; fi\n\
if [ \"$1\" = \"ci\" ] || [ \"$1\" = \"install\" ]; then touch npm_install_called; exit 0; fi\n\
exit 1",
        );

        let root = temp_dir.path().join("workspace");
        let backend_dir = root.join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");
        std::fs::create_dir_all(root.join("node_modules")).expect("node_modules dir");
        std::fs::write(root.join("node_modules/.package-lock.json"), "").expect("lockfile");

        preflight_with(
            &root,
            &backend_dir,
            node_path.as_os_str(),
            npm_path.as_os_str(),
            npx_path.as_os_str(),
        )
        .await
        .expect("preflight");
        assert!(!root.join("npm_install_called").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preflight_reports_bad_tsx_version() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let node_path = write_fake_executable(&bin_dir, "node", "echo v20.1.0");
        let npx_path = write_fake_executable(&bin_dir, "npx", "echo 4.20.0");
        let npm_path = write_fake_executable(
            &bin_dir,
            "npm",
            "if [ \"$1\" = \"--version\" ]; then echo 9.6.0; exit 0; fi",
        );

        let root = temp_dir.path().join("workspace");
        let backend_dir = root.join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");
        std::fs::create_dir_all(root.join("node_modules")).expect("node_modules dir");
        std::fs::write(root.join("node_modules/.package-lock.json"), "").expect("lockfile");

        let err = preflight_with(
            &root,
            &backend_dir,
            node_path.as_os_str(),
            npm_path.as_os_str(),
            npx_path.as_os_str(),
        )
        .await
        .expect_err("preflight should fail on bad tsx");
        let msg = format!("{err}");
        assert!(msg.contains("tsx 4.20.0"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_npm_install_uses_ci_with_lockfile() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let root = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace dir");
        std::fs::write(root.join("package-lock.json"), "{}").expect("lockfile");

        let npm_path = write_fake_executable(&bin_dir, "npm", r#"echo "$1" > "$PWD/command.txt""#);

        run_npm_install_with(&root, npm_path.as_os_str())
            .await
            .expect("npm ci");

        let command = std::fs::read_to_string(root.join("command.txt")).expect("read command");
        assert_eq!(command.trim(), "ci");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_npm_install_uses_install_without_lockfile() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let root = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace dir");

        let npm_path = write_fake_executable(&bin_dir, "npm", r#"echo "$1" > "$PWD/command.txt""#);

        run_npm_install_with(&root, npm_path.as_os_str())
            .await
            .expect("npm install");

        let command = std::fs::read_to_string(root.join("command.txt")).expect("read command");
        assert_eq!(command.trim(), "install");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_npm_install_reports_failure() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let root = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&root).expect("workspace dir");

        let npm_path = write_fake_executable(&bin_dir, "npm", r#"echo "boom" 1>&2; exit 1"#);

        let err = run_npm_install_with(&root, npm_path.as_os_str())
            .await
            .expect_err("npm install failure");
        let msg = format!("{err}");
        assert!(msg.contains("npm install failed"), "msg: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_tsx_version_with_missing_binary_is_ok() {
        let temp_dir = TempDir::new().expect("temp dir");
        let backend_dir = temp_dir.path().join("server");
        std::fs::create_dir_all(&backend_dir).expect("backend dir");

        assert!(check_tsx_version_with(&backend_dir, OsStr::new("missing-npx-123")).is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn forward_output_notifies_on_stdout_pattern() {
        let mut child = AsyncCommand::new("sh")
            .arg("-c")
            .arg("echo READY")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn sh");

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        let ready = std::sync::Arc::new(Notify::new());
        let ready_clone = ready.clone();

        let forward_task = tokio::spawn(async move {
            forward_output(stdout, stderr, "test", "READY", ready_clone).await;
        });

        tokio::time::timeout(Duration::from_secs(2), ready.notified())
            .await
            .expect("ready notification");

        let _ = child.wait().await;
        forward_task.await.expect("forward task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn forward_output_notifies_on_stderr_pattern() {
        let mut child = AsyncCommand::new("sh")
            .arg("-c")
            .arg("echo READY 1>&2")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn sh");

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        let ready = std::sync::Arc::new(Notify::new());
        let ready_clone = ready.clone();

        let forward_task = tokio::spawn(async move {
            forward_output(stdout, stderr, "test", "READY", ready_clone).await;
        });

        tokio::time::timeout(Duration::from_secs(2), ready.notified())
            .await
            .expect("ready notification");

        let _ = child.wait().await;
        forward_task.await.expect("forward task");
    }

    #[tokio::test]
    async fn execute_invalid_workspace_returns_error() {
        let temp_dir = TempDir::new().expect("temp dir");
        let missing = temp_dir.path().join("missing");
        let args = WebArgs {
            backend_port: 3000,
            frontend_port: 5173,
            workspace: Some(missing),
            no_open: true,
        };

        let err = execute(args).await.expect_err("invalid workspace");
        assert!(err.to_string().contains("Invalid workspace path"));
    }
}

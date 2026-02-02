#[cfg(unix)]
mod web_integration {
    use std::fs;
    use std::net::TcpListener;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use tempfile::TempDir;

    fn write_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        let script = format!("#!/bin/sh\n{}\n", body);
        fs::write(&path, script).expect("write script");
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    #[test]
    fn test_web_command_with_fake_node_tools() {
        let temp_dir = TempDir::new().expect("temp dir");
        let workspace = temp_dir.path();

        fs::create_dir_all(workspace.join("backend/ralph-web-server")).expect("backend dir");
        fs::create_dir_all(workspace.join("frontend/ralph-web")).expect("frontend dir");
        fs::create_dir_all(workspace.join("node_modules")).expect("node_modules dir");
        fs::write(workspace.join("node_modules/.package-lock.json"), "").expect("lockfile");

        let bin_dir = workspace.join("bin");
        fs::create_dir_all(&bin_dir).expect("bin dir");

        write_executable(&bin_dir, "node", "echo v20.0.0");
        write_executable(&bin_dir, "npx", "echo 4.21.0");
        write_executable(
            &bin_dir,
            "npm",
            "if [ \"$1\" = \"--version\" ]; then echo 9.6.0; exit 0; fi\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"dev\" ]; then\n\
  echo \"Server started on http://localhost\"\n\
  echo \"Local: http://localhost\"\n\
  sleep 0.1\n\
  exit 0\n\
fi\n\
if [ \"$1\" = \"ci\" ] || [ \"$1\" = \"install\" ]; then exit 0; fi\n\
exit 1",
        );

        let original_path = std::env::var("PATH").unwrap_or_default();
        let path = format!("{}:{}", bin_dir.display(), original_path);

        let backend_port = match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                let port = listener.local_addr().expect("addr").port();
                drop(listener);
                port
            }
            Err(_) => return, // Skip if binding isn't allowed in this environment
        };

        let frontend_port = match TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                let port = listener.local_addr().expect("addr").port();
                drop(listener);
                port
            }
            Err(_) => return,
        };

        let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
            .current_dir(workspace)
            .env("PATH", path)
            .args([
                "web",
                "--workspace",
                workspace.to_str().expect("workspace path"),
                "--no-open",
                "--backend-port",
                &backend_port.to_string(),
                "--frontend-port",
                &frontend_port.to_string(),
            ])
            .output()
            .expect("execute ralph web");

        assert!(
            output.status.success(),
            "ralph web failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Starting Ralph web servers"));
    }
}

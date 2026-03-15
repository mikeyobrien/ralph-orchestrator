use std::process::Command;
use tempfile::TempDir;

fn run_ralph(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("execute ralph")
}

fn run_ralph_with_home(
    temp_path: &std::path::Path,
    home_path: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
        .output()
        .expect("execute ralph")
}

#[test]
fn test_run_dry_run_succeeds() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &[
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello world",
            "--completion-promise",
            "done",
            "--max-iterations",
            "1",
            "--backend",
            "claude",
            "--no-tui",
        ],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dry run mode"), "stdout: {stdout}");
}

#[test]
fn test_run_dry_run_uses_user_scoped_config_defaults() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let user_config_dir = home_dir.path().join(".ralph");
    std::fs::create_dir_all(&user_config_dir).expect("create user config dir");
    std::fs::write(
        user_config_dir.join("config.yml"),
        r"
cli:
  backend: claude
event_loop:
  max_iterations: 7
",
    )
    .expect("write user config");

    let output = run_ralph_with_home(
        temp_path,
        home_dir.path(),
        &[
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello world",
            "--no-tui",
        ],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Backend: claude"), "stdout: {stdout}");
    assert!(stdout.contains("Max iterations: 7"), "stdout: {stdout}");
}

#[test]
fn test_run_dry_run_local_config_overrides_user_scoped_defaults() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let user_config_dir = home_dir.path().join(".ralph");
    std::fs::create_dir_all(&user_config_dir).expect("create user config dir");
    std::fs::write(
        user_config_dir.join("config.yml"),
        r"
cli:
  backend: claude
event_loop:
  max_iterations: 7
",
    )
    .expect("write user config");
    std::fs::write(
        temp_path.join("ralph.yml"),
        r"
cli:
  backend: gemini
event_loop:
  max_iterations: 3
",
    )
    .expect("write local config");

    let output = run_ralph_with_home(
        temp_path,
        home_dir.path(),
        &[
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello world",
            "--no-tui",
        ],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Backend: gemini"), "stdout: {stdout}");
    assert!(stdout.contains("Max iterations: 3"), "stdout: {stdout}");
}

#[test]
fn test_run_smoke_executes_user_scoped_hook_during_real_loop() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let user_config_dir = home_dir.path().join(".ralph");
    std::fs::create_dir_all(&user_config_dir).expect("create user config dir");

    let hook_marker = temp_path.join("global-hook-ran.txt");
    let hook_script = temp_path.join("global-hook.sh");
    let backend_script = temp_path.join("backend-complete.sh");

    std::fs::write(
        &hook_script,
        format!(
            "#!/bin/sh\nprintf 'hook ran' > \"{}\"\n",
            hook_marker.display()
        ),
    )
    .expect("write hook script");
    std::fs::write(
        &backend_script,
        format!(
            "#!/bin/sh\ncat >/dev/null\n\"{}\" emit LOOP_COMPLETE smoke-done\n",
            env!("CARGO_BIN_EXE_ralph")
        ),
    )
    .expect("write backend script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for path in [&hook_script, &backend_script] {
            let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).expect("set executable permissions");
        }
    }

    std::fs::write(
        user_config_dir.join("config.yml"),
        r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: global-hook
        command: ["./global-hook.sh"]
        on_error: warn
"#,
    )
    .expect("write user config");

    std::fs::write(
        temp_path.join("ralph.yml"),
        r#"
cli:
  backend: custom
  command: "./backend-complete.sh"
  prompt_mode: stdin
event_loop:
  max_iterations: 3
  max_runtime_seconds: 10
"#,
    )
    .expect("write local config");

    let output = run_ralph_with_home(
        temp_path,
        home_dir.path(),
        &["run", "--no-tui", "--prompt", "smoke test"],
    );

    assert!(
        output.status.success(),
        "run failed: {}\nstdout:{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        hook_marker.exists(),
        "expected global hook marker at {}",
        hook_marker.display()
    );
}

#[test]
fn test_run_smoke_local_hook_overrides_user_scoped_hook_during_real_loop() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let user_config_dir = home_dir.path().join(".ralph");
    std::fs::create_dir_all(&user_config_dir).expect("create user config dir");

    let global_marker = temp_path.join("global-hook-ran.txt");
    let local_marker = temp_path.join("local-hook-ran.txt");
    let global_hook_script = temp_path.join("global-hook.sh");
    let local_hook_script = temp_path.join("local-hook.sh");
    let backend_script = temp_path.join("backend-complete.sh");

    std::fs::write(
        &global_hook_script,
        format!(
            "#!/bin/sh\nprintf 'global hook ran' > \"{}\"\n",
            global_marker.display()
        ),
    )
    .expect("write global hook script");
    std::fs::write(
        &local_hook_script,
        format!(
            "#!/bin/sh\nprintf 'local hook ran' > \"{}\"\n",
            local_marker.display()
        ),
    )
    .expect("write local hook script");
    std::fs::write(
        &backend_script,
        format!(
            "#!/bin/sh\ncat >/dev/null\n\"{}\" emit LOOP_COMPLETE smoke-done\n",
            env!("CARGO_BIN_EXE_ralph")
        ),
    )
    .expect("write backend script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for path in [&global_hook_script, &local_hook_script, &backend_script] {
            let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).expect("set executable permissions");
        }
    }

    std::fs::write(
        user_config_dir.join("config.yml"),
        r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: global-hook
        command: ["./global-hook.sh"]
        on_error: warn
"#,
    )
    .expect("write user config");

    std::fs::write(
        temp_path.join("ralph.yml"),
        r#"
cli:
  backend: custom
  command: "./backend-complete.sh"
  prompt_mode: stdin
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: local-hook
        command: ["./local-hook.sh"]
        on_error: warn
event_loop:
  max_iterations: 3
  max_runtime_seconds: 10
"#,
    )
    .expect("write local config");

    let output = run_ralph_with_home(
        temp_path,
        home_dir.path(),
        &["run", "--no-tui", "--prompt", "smoke override test"],
    );

    assert!(
        output.status.success(),
        "run failed: {}\nstdout:{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        local_marker.exists(),
        "expected local hook marker at {}",
        local_marker.display()
    );
    assert!(
        !global_marker.exists(),
        "did not expect global hook marker at {}",
        global_marker.display()
    );
}

#[cfg(unix)]
#[test]
fn test_run_autonomous_mode_does_not_apply_adapter_wall_clock_timeout() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();
    let backend_script = temp_path.join("slow-complete.sh");

    std::fs::write(
        &backend_script,
        format!(
            "#!/bin/sh\ncat >/dev/null\nprintf 'heartbeat-1\\n'\nsleep 0.4\nprintf 'heartbeat-2\\n'\nsleep 0.4\nprintf 'heartbeat-3\\n'\nsleep 0.4\n\"{}\" emit LOOP_COMPLETE autonomous-done\n",
            env!("CARGO_BIN_EXE_ralph")
        ),
    )
    .expect("write backend script");

    let mut permissions = std::fs::metadata(&backend_script)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&backend_script, permissions).expect("set executable permissions");

    std::fs::write(
        temp_path.join("ralph.yml"),
        r#"
cli:
  backend: custom
  command: "./slow-complete.sh"
  prompt_mode: stdin
adapters:
  claude:
    timeout: 1
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 1
  max_runtime_seconds: 10
"#,
    )
    .expect("write config");

    let output = run_ralph(
        temp_path,
        &[
            "run",
            "--autonomous",
            "--skip-preflight",
            "--prompt",
            "slow autonomous test",
        ],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("heartbeat-3"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Event emitted: LOOP_COMPLETE"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("Execution inactivity timeout reached, sending SIGTERM"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_run_continue_requires_scratchpad() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &["run", "--continue", "--dry-run", "--skip-preflight"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot continue: scratchpad not found"),
        "stderr: {stderr}"
    );
}

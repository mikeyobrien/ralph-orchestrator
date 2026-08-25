//! Bridge Ralph's RObot services to Autoloop's subprocess control protocol.
//!
//! Autoloop emits `ask.pending` on its structured event stream and blocks until
//! `autoloop control respond` supplies an answer. Ralph keeps that stream as the
//! active `.ralph/current-events` file so Telegram/Web inbound messages land in
//! the same append-only file as `human.response` / `human.guidance` events.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use ralph_adapters::{AutoloopEvent, AutoloopRunner};
use ralph_core::{LoopContext, RalphConfig};
use ralph_proto::RobotService;
use tracing::{info, warn};

use crate::web_robot_service::WebRobotService;

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Restores the prior active-events marker when an Autoloop run ends.
pub(crate) struct CurrentEventsGuard {
    marker: PathBuf,
    previous: Option<Vec<u8>>,
}

impl CurrentEventsGuard {
    pub(crate) fn install(workspace: &Path, events_path: &Path) -> Result<Self> {
        let ralph_dir = workspace.join(".ralph");
        fs::create_dir_all(&ralph_dir)
            .with_context(|| format!("creating {}", ralph_dir.display()))?;
        let marker = ralph_dir.join("current-events");
        let previous = fs::read(&marker).ok();
        let target = events_path
            .strip_prefix(workspace)
            .unwrap_or(events_path)
            .to_string_lossy();
        fs::write(&marker, format!("{target}\n"))
            .with_context(|| format!("writing {}", marker.display()))?;
        Ok(Self { marker, previous })
    }
}

impl Drop for CurrentEventsGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(contents) => {
                if let Err(error) = fs::write(&self.marker, contents) {
                    warn!(error = %error, marker = %self.marker.display(), "Failed to restore current-events marker");
                }
            }
            None => {
                if let Err(error) = fs::remove_file(&self.marker)
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    warn!(error = %error, marker = %self.marker.display(), "Failed to remove current-events marker");
                }
            }
        }
    }
}

/// Create and start the configured primary-loop RObot service.
pub(crate) fn create_robot_service(
    config: &RalphConfig,
    context: &LoopContext,
) -> Option<Box<dyn RobotService>> {
    if !config.robot.enabled || !context.is_primary() {
        return None;
    }

    let workspace_root = context.workspace().to_path_buf();
    let timeout_secs = config.robot.timeout_seconds.unwrap_or(300);
    let loop_id = context
        .loop_id()
        .map(String::from)
        .unwrap_or_else(|| "main".to_string());

    if config.robot.mode.is_web() {
        let service = WebRobotService::new(workspace_root, timeout_secs, loop_id);
        if let Err(error) = service.start() {
            warn!(error = %error, "Failed to start web RObot service");
            return None;
        }
        info!(
            timeout_secs = service.timeout_secs(),
            "Web RObot service active for Autoloop"
        );
        return Some(Box::new(service));
    }

    match ralph_telegram::TelegramService::new(
        workspace_root,
        config.robot.resolve_bot_token(),
        config.robot.resolve_api_url(),
        timeout_secs,
        loop_id,
    ) {
        Ok(service) => {
            if let Err(error) = service.start() {
                warn!(error = %error, "Failed to start Telegram RObot service");
                return None;
            }
            info!(
                bot_token = %service.bot_token_masked(),
                timeout_secs,
                "Telegram RObot service active for Autoloop"
            );
            Some(Box::new(service))
        }
        Err(error) => {
            warn!(error = %error, "Failed to create Telegram RObot service");
            None
        }
    }
}

/// Relay Autoloop asks and Ralph guidance until the subprocess has stopped.
///
/// This function is blocking by design and must run on `spawn_blocking`.
pub(crate) fn run_bridge(
    service: Box<dyn RobotService>,
    runner: AutoloopRunner,
    events_path: PathBuf,
    done: Arc<AtomicBool>,
) -> Result<()> {
    let result = run_bridge_inner(service.as_ref(), &runner, &events_path, &done);
    service.stop();
    result
}

fn run_bridge_inner(
    service: &dyn RobotService,
    runner: &AutoloopRunner,
    events_path: &Path,
    done: &AtomicBool,
) -> Result<()> {
    let mut position = 0;
    let mut run_id: Option<String> = None;
    let mut handled_questions = std::collections::HashSet::new();
    let mut stop_forwarded = false;
    let mut restart_forwarded = false;

    loop {
        let lines = read_complete_lines(events_path, &mut position)?;
        let had_lines = !lines.is_empty();
        for line in lines {
            if let Ok(event) = serde_json::from_str::<AutoloopEvent>(&line) {
                if let Some(id) = event.run_id.as_ref() {
                    run_id = Some(id.clone());
                }
                if let Some(ask) = event.ask_pending()
                    && handled_questions.insert(ask.question_id.clone())
                {
                    let response_start = fs::metadata(events_path).map(|m| m.len()).unwrap_or(0);
                    service.send_question(&ask.question).with_context(|| {
                        format!("sending Autoloop question {}", ask.question_id)
                    })?;
                    if let Some(answer) = wait_for_response_or_control(
                        service,
                        runner,
                        events_path,
                        &ask.run_id,
                        response_start,
                        done,
                        &mut stop_forwarded,
                        &mut restart_forwarded,
                    )
                    .with_context(|| format!("waiting for Autoloop question {}", ask.question_id))?
                    {
                        runner
                            .respond(&ask.run_id, &ask.question_id, &answer)
                            .with_context(|| {
                                format!("responding to Autoloop question {}", ask.question_id)
                            })?;
                    }
                }
                continue;
            }

            let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            if event.get("topic").and_then(|value| value.as_str()) == Some("human.guidance")
                && let Some(id) = run_id.as_deref()
                && let Some(message) = event.get("payload").and_then(|value| value.as_str())
            {
                runner
                    .guide(id, message)
                    .context("forwarding human guidance to Autoloop")?;
            }
        }

        if let Some(id) = run_id.as_deref() {
            let ralph_dir = events_path
                .parent()
                .context("Autoloop events path has no .ralph parent")?;
            if !stop_forwarded && ralph_dir.join("stop-requested").exists() {
                runner
                    .interrupt(id, "Ralph stop requested")
                    .context("forwarding stop request to Autoloop")?;
                stop_forwarded = true;
            }
            if !restart_forwarded && ralph_dir.join("restart-requested").exists() {
                runner
                    .interrupt(id, "Ralph restart requested")
                    .context("forwarding restart request to Autoloop")?;
                restart_forwarded = true;
            }
        }

        if done.load(Ordering::Acquire) {
            // A quiet pass after process exit proves the final append was drained.
            if !had_lines {
                return Ok(());
            }
            continue;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[allow(clippy::too_many_arguments)]
fn wait_for_response_or_control(
    service: &dyn RobotService,
    runner: &AutoloopRunner,
    events_path: &Path,
    run_id: &str,
    response_start: u64,
    done: &AtomicBool,
    stop_forwarded: &mut bool,
    restart_forwarded: &mut bool,
) -> Result<Option<String>> {
    let shutdown = service.shutdown_flag();
    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| service.wait_for_response(events_path, Some(response_start)));
        while !waiter.is_finished() {
            let ralph_dir = events_path
                .parent()
                .context("Autoloop events path has no .ralph parent")?;
            if !*stop_forwarded && ralph_dir.join("stop-requested").exists() {
                runner
                    .interrupt(run_id, "Ralph stop requested")
                    .context("forwarding stop request to Autoloop")?;
                *stop_forwarded = true;
                shutdown.store(true, Ordering::Release);
            }
            if !*restart_forwarded && ralph_dir.join("restart-requested").exists() {
                runner
                    .interrupt(run_id, "Ralph restart requested")
                    .context("forwarding restart request to Autoloop")?;
                *restart_forwarded = true;
                shutdown.store(true, Ordering::Release);
            }
            if done.load(Ordering::Acquire) {
                shutdown.store(true, Ordering::Release);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        waiter
            .join()
            .map_err(|_| anyhow::anyhow!("RObot response waiter panicked"))?
    })
}

fn read_complete_lines(path: &Path, position: &mut u64) -> Result<Vec<String>> {
    let Ok(mut file) = File::open(path) else {
        return Ok(Vec::new());
    };
    file.seek(SeekFrom::Start(*position))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if !line.ends_with('\n') {
            break;
        }
        *position += bytes as u64;
        let line = line.trim();
        if !line.is_empty() {
            lines.push(line.to_string());
        }
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::CheckinContext;
    use std::io::Write;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeState {
        questions: Mutex<Vec<String>>,
        stopped: AtomicBool,
    }

    struct FakeRobot {
        state: Arc<FakeState>,
        shutdown: Arc<AtomicBool>,
    }

    impl RobotService for FakeRobot {
        fn send_question(&self, payload: &str) -> anyhow::Result<i32> {
            self.state
                .questions
                .lock()
                .unwrap()
                .push(payload.to_string());
            Ok(7)
        }

        fn wait_for_response(
            &self,
            events_path: &Path,
            start_position: Option<u64>,
        ) -> anyhow::Result<Option<String>> {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let mut position = start_position.unwrap_or(0);
            while std::time::Instant::now() < deadline {
                if self.shutdown.load(Ordering::Acquire) {
                    return Ok(None);
                }
                if let Ok(content) = fs::read_to_string(events_path) {
                    let start = usize::try_from(position)
                        .unwrap_or(content.len())
                        .min(content.len());
                    for line in content[start..].lines() {
                        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
                            continue;
                        };
                        if event.get("topic").and_then(|value| value.as_str())
                            == Some("human.response")
                        {
                            return Ok(event
                                .get("payload")
                                .and_then(|value| value.as_str())
                                .map(str::to_string));
                        }
                    }
                    position = content.len() as u64;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None)
        }

        fn send_checkin(
            &self,
            _iteration: u32,
            _elapsed: Duration,
            _context: Option<&CheckinContext>,
        ) -> anyhow::Result<i32> {
            Ok(0)
        }

        fn timeout_secs(&self) -> u64 {
            2
        }

        fn shutdown_flag(&self) -> Arc<AtomicBool> {
            self.shutdown.clone()
        }

        fn stop(self: Box<Self>) {
            self.state.stopped.store(true, Ordering::Release);
            self.shutdown.store(true, Ordering::Release);
        }
    }

    #[cfg(unix)]
    #[test]
    fn relays_pending_ask_response_and_guidance_through_control_cli_once() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let events_path = temp.path().join("events.ndjson");
        let control_log = temp.path().join("control.log");
        let fake_bin = temp.path().join("fake-autoloop.sh");
        fs::write(
            &fake_bin,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CONTROL_LOG\"\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_bin).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_bin, permissions).unwrap();

        let runner = AutoloopRunner::new(temp.path(), "prompt", temp.path())
            .bin(ralph_adapters::AutoloopBin::Explicit(fake_bin))
            .env("CONTROL_LOG", control_log.to_string_lossy().into_owned());
        let state = Arc::new(FakeState::default());
        let robot: Box<dyn RobotService> = Box::new(FakeRobot {
            state: state.clone(),
            shutdown: Arc::new(AtomicBool::new(false)),
        });
        let done = Arc::new(AtomicBool::new(false));

        let bridge_done = done.clone();
        let bridge_events = events_path.clone();
        let bridge = std::thread::spawn(move || {
            run_bridge(robot, runner, bridge_events, bridge_done).unwrap();
        });

        let mut events = File::create(&events_path).unwrap();
        writeln!(
            events,
            r#"{{"type":"iteration.start","runId":"run-9","iteration":1}}"#
        )
        .unwrap();
        writeln!(
            events,
            r#"{{"topic":"human.guidance","payload":"check cancellation"}}"#
        )
        .unwrap();
        writeln!(
            events,
            r#"{{"type":"ask.pending","runId":"run-9","iteration":1,"questionId":"ask-9","question":"A or B?"}}"#
        )
        .unwrap();
        events.flush().unwrap();
        let response_path = events_path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(response_path)
                .unwrap();
            writeln!(file, r#"{{"topic":"human.response","payload":"Use A"}}"#).unwrap();
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            let log = fs::read_to_string(&control_log).unwrap_or_default();
            if log.contains("control respond run-9 ask-9 Use A") {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        done.store(true, Ordering::Release);
        bridge.join().unwrap();

        let log = fs::read_to_string(control_log).unwrap();
        assert_eq!(
            log.matches("control guide run-9 check cancellation")
                .count(),
            1
        );
        assert_eq!(log.matches("control respond run-9 ask-9 Use A").count(), 1);
        assert_eq!(state.questions.lock().unwrap().as_slice(), ["A or B?"]);
        assert!(state.stopped.load(Ordering::Acquire));
    }

    #[cfg(unix)]
    #[test]
    fn restart_interrupts_an_pending_ask_without_waiting_for_hitl_timeout() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let events_path = temp.path().join("events.ndjson");
        let control_log = temp.path().join("control.log");
        let fake_bin = temp.path().join("fake-autoloop.sh");
        fs::write(
            &fake_bin,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CONTROL_LOG\"\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&fake_bin).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_bin, permissions).unwrap();

        let runner = AutoloopRunner::new(temp.path(), "prompt", temp.path())
            .bin(ralph_adapters::AutoloopBin::Explicit(fake_bin))
            .env("CONTROL_LOG", control_log.to_string_lossy().into_owned());
        let state = Arc::new(FakeState::default());
        let robot: Box<dyn RobotService> = Box::new(FakeRobot {
            state,
            shutdown: Arc::new(AtomicBool::new(false)),
        });
        let done = Arc::new(AtomicBool::new(false));
        fs::write(
            &events_path,
            concat!(
                r#"{"type":"iteration.start","runId":"run-9","iteration":1}"#,
                "\n",
                r#"{"type":"ask.pending","runId":"run-9","iteration":1,"questionId":"ask-9","question":"A or B?"}"#,
                "\n",
            ),
        )
        .unwrap();

        let bridge_done = done.clone();
        let bridge_events = events_path.clone();
        let bridge = std::thread::spawn(move || {
            run_bridge(robot, runner, bridge_events, bridge_done).unwrap();
        });
        std::thread::sleep(Duration::from_millis(100));
        fs::write(temp.path().join("restart-requested"), "").unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while std::time::Instant::now() < deadline {
            if fs::read_to_string(&control_log)
                .unwrap_or_default()
                .contains("control interrupt run-9 --reason Ralph restart requested")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        done.store(true, Ordering::Release);
        bridge.join().unwrap();

        let log = fs::read_to_string(control_log).unwrap();
        assert_eq!(
            log.matches("control interrupt run-9 --reason Ralph restart requested")
                .count(),
            1
        );
        assert!(!log.contains("control respond"));
    }

    #[test]
    fn current_events_guard_restores_previous_marker() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join(".ralph/current-events");
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, ".ralph/old.jsonl\n").unwrap();
        let events = temp.path().join(".ralph/autoloop-events.ndjson");

        {
            let _guard = CurrentEventsGuard::install(temp.path(), &events).unwrap();
            assert_eq!(
                fs::read_to_string(&marker).unwrap(),
                ".ralph/autoloop-events.ndjson\n"
            );
        }

        assert_eq!(fs::read_to_string(marker).unwrap(), ".ralph/old.jsonl\n");
    }
}

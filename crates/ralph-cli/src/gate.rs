//! `ralph gate`: judgments the autoloop engine runs at its completion seam.
//!
//! The engine runs `ralph gate jev-judge` as an acceptance `verify_cmd` on
//! every done-claim and holds completion unless it exits 0. The one stdout line
//! is what the engine journals (`acceptance.command.output_tail`) and hands
//! back to the agent when completion is held, so it carries the deciding
//! values and nothing else.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ralph_adapters::replay_journal;
use ralph_core::jev_judge::{
    JudgeSnapshot, JudgeState, judge, request_judgment, summary_line, telemetry,
};

// `ralph gate jev-route` is the engine's pre_emit hook in Jev topology mode;
// see `ralph_core::jev_topology` for the seam.

#[derive(Parser, Debug)]
pub struct GateArgs {
    #[command(subcommand)]
    pub command: GateCommands,
}

#[derive(Subcommand, Debug)]
pub enum GateCommands {
    /// Judge the current done-claim with Jev (run by the engine).
    JevJudge {
        /// Judge snapshot Ralph wrote when the run started.
        #[arg(long)]
        snapshot: PathBuf,
    },
    /// Choose the next role with Jev (run by the engine as a pre_emit hook).
    JevRoute {
        /// Route snapshot Ralph wrote when the run started.
        #[arg(long)]
        snapshot: PathBuf,
    },
}

pub async fn execute(args: GateArgs) -> Result<()> {
    match args.command {
        GateCommands::JevJudge { snapshot } => jev_judge(&snapshot).await,
        GateCommands::JevRoute { snapshot } => jev_route(&snapshot).await,
    }
}

async fn jev_judge(snapshot_path: &Path) -> Result<()> {
    let snapshot = JudgeSnapshot::read(snapshot_path)
        .with_context(|| format!("reading judge snapshot {}", snapshot_path.display()))?;
    let (run_id, state) = judge_state(&snapshot);
    let decision = judge(
        &snapshot.settings,
        &state,
        std::env::var("TYPESAFE_API_KEY").ok(),
        |key, body, timeout| async move { request_judgment(&key, &body, timeout).await },
    )
    .await;

    let record = telemetry(&decision, run_id.as_deref());
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&snapshot.record_file)
    {
        let _ = writeln!(file, "{record}");
    }
    println!("{}", summary_line(&decision));
    if decision.approved {
        Ok(())
    } else {
        // Exit non-zero without anyhow's error chain: the line above is the
        // whole message the engine should journal.
        std::process::exit(1);
    }
}

/// The engine's `pre_emit` hook in Jev topology mode. Reads the emitted event
/// from `AUTOLOOP_EMIT_TOPIC`/`AUTOLOOP_EMIT_PAYLOAD`; prints the mutation
/// directive the engine applies, or exits 1 to block the handoff.
async fn jev_route(snapshot_path: &Path) -> Result<()> {
    use ralph_core::jev_topology::{
        EmitAction, RouteFailure, RouteSnapshot, RouteState, classify_emit, directive,
        parse_choice, request_body,
    };

    let snapshot = RouteSnapshot::read(snapshot_path)
        .with_context(|| format!("reading route snapshot {}", snapshot_path.display()))?;
    let topic = std::env::var("AUTOLOOP_EMIT_TOPIC").unwrap_or_default();
    let payload = std::env::var("AUTOLOOP_EMIT_PAYLOAD").unwrap_or_default();

    let outcome = match classify_emit(&topic, &snapshot.completion_event) {
        EmitAction::PassThrough => return Ok(()),
        EmitAction::Block(message) => Err(message),
        EmitAction::Route => {
            let (run_id, objective, finished_role, recent_steps) = route_context(&snapshot);
            let state = RouteState {
                objective,
                finished_role: finished_role.clone(),
                step_summary: payload.clone(),
                recent_steps,
            };
            let result = match std::env::var("TYPESAFE_API_KEY")
                .ok()
                .filter(|key| !key.trim().is_empty())
            {
                None => Err(RouteFailure::Provider(
                    ralph_core::jev_judge::JudgeFailure::MissingCredential,
                )),
                Some(key) => {
                    let body = request_body(&snapshot.settings, &snapshot.roles, &state);
                    let timeout = std::time::Duration::from_millis(snapshot.settings.timeout_ms);
                    match request_judgment(&key, &body, timeout).await {
                        Ok(response) => parse_choice(
                            &response,
                            &snapshot.roles,
                            snapshot.settings.min_confidence,
                        ),
                        Err(failure) => Err(RouteFailure::Provider(failure)),
                    }
                }
            };
            let record = match &result {
                Ok(choice) => serde_json::json!({
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "run_id": run_id,
                    "from": finished_role,
                    "routed": true,
                    "choice": choice,
                }),
                Err(failure) => serde_json::json!({
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "run_id": run_id,
                    "from": finished_role,
                    "routed": false,
                    "reason": failure.message(),
                }),
            };
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&snapshot.record_file)
            {
                let _ = writeln!(file, "{record}");
            }
            result
                .map(|choice| directive(&choice, &payload))
                .map_err(|failure| failure.message())
        }
    };
    match outcome {
        Ok(directive) => {
            println!("{directive}");
            Ok(())
        }
        Err(message) => {
            // The engine journals this line and hands it back to the agent.
            println!("{message}");
            std::process::exit(1);
        }
    }
}

/// Run id, objective, the role that just finished, and recent routed steps.
fn route_context(
    snapshot: &ralph_core::jev_topology::RouteSnapshot,
) -> (Option<String>, String, String, Vec<String>) {
    let records = std::fs::read_to_string(&snapshot.journal_file)
        .ok()
        .and_then(|content| replay_journal(&content).ok())
        .unwrap_or_default();
    let start = records
        .iter()
        .rposition(|record| record.topic == "loop.start");
    let run = start.map(|index| &records[index..]).unwrap_or_default();
    let run_id = run.first().map(|record| record.run.clone());
    let objective = run
        .first()
        .and_then(|record| record.field("objective"))
        .unwrap_or_default()
        .to_string();
    let finished_role = run
        .iter()
        .rev()
        .find(|record| record.topic == "iteration.start")
        .and_then(|record| record.field("suggested_roles"))
        .and_then(|roles| roles.split(',').next())
        .unwrap_or_default()
        .to_string();
    let recent_steps = run
        .iter()
        .filter(|record| {
            record
                .topic
                .starts_with(ralph_core::jev_topology::ROUTE_PREFIX)
        })
        .map(|record| record.topic.clone())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (run_id, objective, finished_role, recent_steps)
}

/// The current run's objective, latest claim, latest output, and changes.
fn judge_state(snapshot: &JudgeSnapshot) -> (Option<String>, JudgeState) {
    let records = std::fs::read_to_string(&snapshot.journal_file)
        .ok()
        .and_then(|content| replay_journal(&content).ok())
        .unwrap_or_default();
    let run_start = records
        .iter()
        .rposition(|record| record.topic == "loop.start");
    let run = run_start.map(|index| &records[index..]).unwrap_or_default();
    let run_id = run.first().map(|record| record.run.clone());
    let field = |topic: &str, key: &str| {
        run.iter()
            .rev()
            .filter(|record| record.topic == topic)
            .find_map(|record| record.field(key).map(str::to_owned))
            .unwrap_or_default()
    };
    let completion_claim = run
        .iter()
        .rev()
        .find_map(|record| {
            record
                .field("payload")
                .map(|payload| format!("{}: {payload}", record.topic))
        })
        .unwrap_or_default();
    let state = JudgeState {
        objective: field("loop.start", "objective"),
        completion_claim,
        recent_output: field("iteration.finish", "output"),
        changes: workspace_changes(&snapshot.workspace),
    };
    (run_id, state)
}

fn workspace_changes(workspace: &Path) -> String {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(workspace)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    };
    format!(
        "{}{}",
        git(&["diff", "--stat", "HEAD"]),
        git(&["status", "--porcelain"])
    )
}

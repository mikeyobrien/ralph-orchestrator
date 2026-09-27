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
}

pub async fn execute(args: GateArgs) -> Result<()> {
    match args.command {
        GateCommands::JevJudge { snapshot } => jev_judge(&snapshot).await,
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

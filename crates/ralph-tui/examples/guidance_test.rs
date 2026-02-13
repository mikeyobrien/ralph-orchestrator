//! Interactive test harness for guidance injection feature.
//!
//! Launches the TUI with mock iteration data so guidance keybindings
//! can be tested interactively via tmux send-keys.
//!
//! Run with: cargo run -p ralph-tui --example guidance_test
//!
//! Events path: /tmp/ralph-guidance-test/events.jsonl

use ralph_proto::HatId;
use ralph_tui::Tui;
use ratatui::text::Line;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up events path (use HOME for Termux compatibility)
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let test_dir = PathBuf::from(home).join(".ralph-guidance-test");
    std::fs::create_dir_all(&test_dir)?;
    let events_path = test_dir.join("events.jsonl");
    // Clean previous test data
    let _ = std::fs::remove_file(&events_path);

    // Create termination channel
    let (_terminated_tx, terminated_rx) = tokio::sync::watch::channel(false);

    // Build TUI
    let tui = Tui::new()
        .with_termination_signal(terminated_rx)
        .with_events_path(events_path.clone());

    let state = tui.state();

    // Seed some mock data
    {
        let mut s = state.lock().unwrap();
        s.max_iterations = Some(10);

        // Create a couple of iterations with content
        s.start_new_iteration();
        if let Some(buf) = s.current_iteration_mut() {
            buf.hat_display = Some("Builder".to_string());
            for i in 0..30 {
                buf.append_line(Line::from(format!(
                    "  [iter 1] Line {}: doing some work...",
                    i
                )));
            }
        }

        s.start_new_iteration();
        if let Some(buf) = s.current_iteration_mut() {
            buf.hat_display = Some("Reviewer".to_string());
            for i in 0..20 {
                buf.append_line(Line::from(format!(
                    "  [iter 2] Line {}: reviewing changes...",
                    i
                )));
            }
        }

        s.iteration = 2;
        s.pending_hat = Some((HatId::new("builder"), "Builder".to_string()));
    }

    // Run the TUI (blocks until q or Ctrl+C)
    tui.run().await?;

    // After TUI exits, dump test results
    eprintln!("\n=== Guidance Test Results ===");

    // Check guidance_next_queue
    {
        let s = state.lock().unwrap();
        let queue = s.guidance_next_queue.lock().unwrap();
        eprintln!("Next-queue messages: {}", queue.len());
        for (i, msg) in queue.iter().enumerate() {
            eprintln!("  [{}]: {}", i, msg);
        }
    }

    // Check events.jsonl for "now" guidance
    if events_path.exists() {
        let content = std::fs::read_to_string(&events_path)?;
        let lines: Vec<&str> = content.lines().collect();
        eprintln!("Events.jsonl lines: {}", lines.len());
        for line in &lines {
            eprintln!("  {}", line);
        }
    } else {
        eprintln!("Events.jsonl: not created (no 'now' guidance sent)");
    }

    eprintln!("=== Done ===");
    Ok(())
}

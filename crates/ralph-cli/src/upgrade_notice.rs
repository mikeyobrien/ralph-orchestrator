//! Tells a person, at most once a day, that a later major version is out.
//!
//! A later major version is not an update. It has breaking changes, and the usual update
//! commands stay on the running major version, so the notice says so and points at the
//! release's page.
//!
//! The notice is for people. It is printed after a run, on stderr, only when stderr is a
//! terminal, and never in RPC mode or in CI. Commands that agents call inside a loop,
//! such as `ralph emit` and `ralph tools`, never check and never print.

use crate::config_resolution::home_dir_from_env;
use ralph_tui::state::MajorRelease;
use ralph_tui::update_check::{self, DISABLE_ENV};
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinHandle;

const DAY_SECS: i64 = 24 * 60 * 60;

/// The check has its own two-second limit and starts with the run, so this wait is
/// almost never used.
const FINISH_WAIT: Duration = Duration::from_secs(3);

/// Kept beside the user's config, in `~/.ralph/`.
const STATE_FILE: &str = "update-check.json";

/// What the last check found, and when the notice was last shown.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NoticeState {
    #[serde(default)]
    checked_at: Option<i64>,
    #[serde(default)]
    new_major: Option<MajorRelease>,
    #[serde(default)]
    notified_at: Option<i64>,
}

/// A run's pending notice: started with [`begin`], ended with [`UpgradeNotice::finish`].
pub struct UpgradeNotice {
    state_path: Option<PathBuf>,
    check: Option<JoinHandle<()>>,
}

/// Starts the daily check in the background when a person is watching this run.
pub fn begin(rpc: bool) -> UpgradeNotice {
    let off = UpgradeNotice {
        state_path: None,
        check: None,
    };
    if rpc || !std::io::stderr().is_terminal() || update_check::update_check_disabled() {
        return off;
    }
    let Some(state_path) = home_dir_from_env().map(|home| home.join(".ralph").join(STATE_FILE))
    else {
        return off;
    };

    let check = check_due(&load(&state_path), now()).then(|| {
        let state_path = state_path.clone();
        tokio::spawn(async move {
            // A check that fails changes nothing, so the next run asks again.
            if let Some(found) = update_check::fetch_update_check().await {
                let mut state = load(&state_path);
                state.checked_at = Some(now());
                state.new_major = found.new_major;
                save(&state_path, &state);
            }
        })
    });

    UpgradeNotice {
        state_path: Some(state_path),
        check,
    }
}

impl UpgradeNotice {
    /// Prints the notice when a later major version is known and none was shown today.
    pub async fn finish(self) {
        let Some(state_path) = self.state_path else {
            return;
        };
        if let Some(check) = self.check {
            let _ = tokio::time::timeout(FINISH_WAIT, check).await;
        }

        let mut state = load(&state_path);
        let now = now();
        let Some(release) = due_notice(&state, now) else {
            return;
        };
        eprintln!("\n{}", notice_text(release, env!("CARGO_PKG_VERSION")));
        state.notified_at = Some(now);
        save(&state_path, &state);
    }
}

fn check_due(state: &NoticeState, now: i64) -> bool {
    a_day_ago_or_never(state.checked_at, now)
}

fn due_notice(state: &NoticeState, now: i64) -> Option<&MajorRelease> {
    let release = state.new_major.as_ref()?;
    a_day_ago_or_never(state.notified_at, now).then_some(release)
}

fn a_day_ago_or_never(at: Option<i64>, now: i64) -> bool {
    // A time ahead of the clock means the clock was changed. Treat it as long ago.
    at.is_none_or(|at| at > now || now - at >= DAY_SECS)
}

fn notice_text(release: &MajorRelease, running: &str) -> String {
    let major = running.split('.').next().unwrap_or(running);
    format!(
        "{title} is out. It is a new major version with breaking changes.\n\
         Updating ralph {major}.x does not install it. Read {url} before you switch.\n\
         Set {DISABLE_ENV}=1 to turn this check off.",
        title = release.title,
        url = release.url,
    )
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// A missing or damaged file reads as nothing known.
fn load(path: &Path) -> NoticeState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Best effort: a notice that cannot be recorded is shown again on the next run.
fn save(path: &Path, state: &NoticeState) {
    let Ok(text) = serde_json::to_string_pretty(state) else {
        return;
    };
    if let Some(directory) = path.parent()
        && std::fs::create_dir_all(directory).is_err()
    {
        return;
    }
    // Loops can run side by side, so each process writes its own temporary file.
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    if std::fs::write(&temporary, text).is_ok() && std::fs::rename(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> MajorRelease {
        MajorRelease {
            version: "3.0.0".to_string(),
            title: "Ralph 3.0.0".to_string(),
            url: "https://github.com/example/ralph/releases/tag/v3.0.0".to_string(),
        }
    }

    #[test]
    fn asks_github_at_most_once_a_day() {
        let never = NoticeState::default();
        assert!(check_due(&never, 1_000_000));

        let checked = NoticeState {
            checked_at: Some(1_000_000),
            ..NoticeState::default()
        };
        assert!(!check_due(&checked, 1_000_000 + DAY_SECS - 1));
        assert!(check_due(&checked, 1_000_000 + DAY_SECS));
        assert!(
            check_due(&checked, 999_000),
            "a check dated ahead of the clock counts as old"
        );
    }

    #[test]
    fn shows_the_notice_once_a_day_and_only_for_a_later_major() {
        let nothing_known = NoticeState {
            checked_at: Some(1_000_000),
            ..NoticeState::default()
        };
        assert_eq!(due_notice(&nothing_known, 1_000_000), None);

        let mut known = NoticeState {
            checked_at: Some(1_000_000),
            new_major: Some(release()),
            notified_at: None,
        };
        assert_eq!(due_notice(&known, 1_000_000), Some(&release()));

        known.notified_at = Some(1_000_000);
        assert_eq!(due_notice(&known, 1_000_000 + DAY_SECS - 1), None);
        assert_eq!(due_notice(&known, 1_000_000 + DAY_SECS), Some(&release()));
    }

    #[test]
    fn the_notice_says_what_is_out_that_it_breaks_and_where_to_read() {
        let text = notice_text(&release(), "2.12.0");
        assert!(text.starts_with("Ralph 3.0.0 is out."));
        assert!(text.contains("breaking changes"));
        assert!(text.contains("Updating ralph 2.x does not install it."));
        assert!(text.contains("https://github.com/example/ralph/releases/tag/v3.0.0"));
        assert!(text.contains("RALPH_NO_UPDATE_CHECK=1"));
    }

    #[test]
    fn state_is_kept_between_runs_and_a_damaged_file_reads_as_nothing_known() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("nested").join(STATE_FILE);
        assert_eq!(load(&path), NoticeState::default());

        let state = NoticeState {
            checked_at: Some(1_000_000),
            new_major: Some(release()),
            notified_at: Some(1_000_100),
        };
        save(&path, &state);
        assert_eq!(load(&path), state);
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().expect("parent"))
            .expect("directory")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(leftovers, [std::ffi::OsString::from(STATE_FILE)]);

        std::fs::write(&path, "{ not json").expect("write");
        assert_eq!(load(&path), NoticeState::default());
    }
}

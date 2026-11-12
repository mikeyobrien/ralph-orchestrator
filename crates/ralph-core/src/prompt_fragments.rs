//! Model-specific prompt fragments.
//!
//! A fragment is instruction text that applies only when a hat (or the
//! coordinator) runs on a model whose ID matches one of its glob patterns.
//! Resolution happens at config load in [`RalphConfig::normalize`]; this
//! module holds the config type, the model glob, and the model lookup that
//! mirrors how the loop runner builds the backend command line.
//!
//! [`RalphConfig::normalize`]: crate::RalphConfig::normalize

use serde::{Deserialize, Serialize};

/// Reserved `hats:` entry that targets the coordinator (hatless) prompt.
pub const COORDINATOR_FRAGMENT_TARGET: &str = "ralph";

/// One entry of the top-level `fragments:` list.
///
/// ```yaml
/// fragments:
///   - models: ["claude-fable-5-2*"]   # glob patterns; required
///     hats: [builder]                 # optional; default: every hat + coordinator
///     text: |
///       You are the owner and long-term maintainer of this codebase.
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FragmentConfig {
    /// Glob patterns (`*`, `?`) matched case-sensitively against the full model ID.
    pub models: Vec<String>,

    /// Hat IDs this fragment targets; `ralph` names the coordinator.
    /// `None` targets every model hat plus the coordinator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hats: Option<Vec<String>>,

    /// Text appended to the target's instructions.
    pub text: String,
}

impl FragmentConfig {
    /// True when any pattern matches `model`.
    pub fn matches_model(&self, model: &str) -> bool {
        self.models.iter().any(|pattern| glob_match(pattern, model))
    }

    /// True when the fragment targets `id` (a hat ID or `ralph`).
    pub fn targets(&self, id: &str) -> bool {
        self.hats
            .as_ref()
            .is_none_or(|hats| hats.iter().any(|h| h == id))
    }

    /// Why this fragment can never apply as written, if anything.
    pub(crate) fn defect(&self) -> Option<String> {
        if self.models.is_empty() {
            return Some("'models' is empty, so it matches no model".to_string());
        }
        if let Some(i) = self.models.iter().position(|p| p.trim().is_empty()) {
            return Some(format!("'models[{i}]' is an empty pattern"));
        }
        if self.text.trim().is_empty() {
            return Some("'text' is empty".to_string());
        }
        match &self.hats {
            Some(hats) if hats.is_empty() => Some(
                "'hats' is an empty list, so it targets nothing (omit 'hats' to target every hat)"
                    .to_string(),
            ),
            Some(hats) => hats
                .iter()
                .position(|h| h.trim().is_empty())
                .map(|i| format!("'hats[{i}]' is an empty hat ID")),
            None => None,
        }
    }
}

/// Case-sensitive glob over the whole string: `*` matches any run of
/// characters (including none), `?` exactly one character.
///
/// Deliberately not `Topic::matches_str`, which matches dot-separated
/// segments and would mis-handle model IDs.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Position of the last `*` seen, and the text index it was tried at.
    let mut backtrack: Option<(usize, usize)> = None;

    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                backtrack = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match backtrack {
                // Let the last `*` swallow one more character and retry.
                Some((star_p, star_t)) => {
                    p = star_p + 1;
                    t = star_t + 1;
                    backtrack = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// What a scan of backend args found about the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgsModel<'a> {
    /// No model flag in the args.
    Absent,
    /// A model flag with its value.
    Pinned(&'a str),
    /// A model flag with no value after it. The backend sees the flag, so
    /// `model:` is not applied either; the model is unknown.
    Dangling,
}

/// Finds the model pinned in backend args: `--model <id>`, `--model=<id>`, or
/// `-m <id>`. The first flag wins, as it does for the backend's own parser
/// in the common case; a pinned flag stops the loop runner from adding one.
pub(crate) fn model_in_args(args: &[String]) -> ArgsModel<'_> {
    for (i, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix("--model=") {
            return if value.is_empty() {
                ArgsModel::Dangling
            } else {
                ArgsModel::Pinned(value)
            };
        }
        if arg == "--model" || arg == "-m" {
            return match args.get(i + 1) {
                Some(value) if !value.is_empty() && !value.starts_with('-') => {
                    ArgsModel::Pinned(value)
                }
                _ => ArgsModel::Dangling,
            };
        }
    }
    ArgsModel::Absent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_wildcards_over_full_id() {
        for (pattern, text, expected) in [
            ("claude-fable-5-2", "claude-fable-5-2", true),
            ("claude-fable-5-2", "claude-fable-5-2-x", false),
            ("claude-fable-*", "claude-fable-5-2", true),
            ("claude-fable-*", "claude-fable-", true),
            ("claude-fable-*", "claude-opus-5-5", false),
            ("*-5-2", "claude-fable-5-2", true),
            ("*-5-2", "claude-fable-5-3", false),
            ("claude-*-5-2", "claude-fable-5-2", true),
            ("claude-*-5-2", "claude-fable-5-2-5-2", true),
            ("claude-*-5-2", "claude-5-2", false),
            ("claude-fable-5-?", "claude-fable-5-2", true),
            ("claude-fable-5-?", "claude-fable-5-", false),
            ("claude-fable-5-?", "claude-fable-5-22", false),
            ("Claude-*", "claude-fable", false),
            ("*", "", true),
            ("*", "anything.with.dots", true),
            ("", "", true),
            ("", "x", false),
            ("a**b", "ab", true),
            ("*a*b*", "xxaxxbxx", true),
            ("*a*b", "xxaxxbxxc", false),
            ("gpt.5", "gpt-5", false),
            ("?", "é", true),
        ] {
            assert_eq!(
                glob_match(pattern, text),
                expected,
                "{pattern:?} vs {text:?}"
            );
        }
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn model_in_args_finds_each_flag_form() {
        assert_eq!(
            model_in_args(&args(&["--verbose", "--model", "m1"])),
            ArgsModel::Pinned("m1")
        );
        assert_eq!(
            model_in_args(&args(&["--model=m2"])),
            ArgsModel::Pinned("m2")
        );
        assert_eq!(
            model_in_args(&args(&["exec", "-m", "m3"])),
            ArgsModel::Pinned("m3")
        );
        assert_eq!(
            model_in_args(&args(&["--model", "first", "--model", "second"])),
            ArgsModel::Pinned("first")
        );
        assert_eq!(model_in_args(&args(&["--verbose"])), ArgsModel::Absent);
        assert_eq!(model_in_args(&args(&["--modelx"])), ArgsModel::Absent);
    }

    #[test]
    fn model_in_args_reports_a_flag_without_a_value() {
        for list in [
            &["--model"][..],
            &["--model="][..],
            &["--model", "--verbose"][..],
            &["-m", ""][..],
        ] {
            assert_eq!(model_in_args(&args(list)), ArgsModel::Dangling, "{list:?}");
        }
    }

    #[test]
    fn defect_names_the_broken_field() {
        let ok = FragmentConfig {
            models: vec!["claude-*".into()],
            hats: None,
            text: "t".into(),
        };
        assert_eq!(ok.defect(), None);
        let cases = [
            (
                FragmentConfig {
                    models: vec![],
                    ..ok.clone()
                },
                "models",
            ),
            (
                FragmentConfig {
                    models: vec![" ".into()],
                    ..ok.clone()
                },
                "models[0]",
            ),
            (
                FragmentConfig {
                    text: "\n".into(),
                    ..ok.clone()
                },
                "text",
            ),
            (
                FragmentConfig {
                    hats: Some(vec![]),
                    ..ok.clone()
                },
                "hats",
            ),
            (
                FragmentConfig {
                    hats: Some(vec!["a".into(), String::new()]),
                    ..ok.clone()
                },
                "hats[1]",
            ),
        ];
        for (fragment, needle) in cases {
            let defect = fragment.defect().expect("defect");
            assert!(defect.contains(needle), "{needle}: {defect}");
        }
    }
}

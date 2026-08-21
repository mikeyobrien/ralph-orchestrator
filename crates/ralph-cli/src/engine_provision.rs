use anyhow::{Context, Result};
use ralph_adapters::AutoloopBin;
use ralph_core::autoloop_health::{AutoloopHealth, VENDORED_AUTOLOOP_VERSION, check_autoloop};
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionDecision {
    PromptUser,
    AutoInstall,
    Refuse,
}

pub fn provisioning_decision(interactive: bool, auto_install_opt_in: bool) -> ProvisionDecision {
    if auto_install_opt_in {
        ProvisionDecision::AutoInstall
    } else if interactive {
        ProvisionDecision::PromptUser
    } else {
        ProvisionDecision::Refuse
    }
}

pub fn auto_install_opt_in() -> bool {
    let value = std::env::var("RALPH_AUTO_INSTALL_ENGINE").ok();
    auto_install_opt_in_value(value.as_deref())
}

fn auto_install_opt_in_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true")
    })
}

fn needs_provisioning(health: &AutoloopHealth, skip_preflight: bool) -> bool {
    match health {
        AutoloopHealth::Missing => true,
        AutoloopHealth::TooOld { .. } => !skip_preflight,
        AutoloopHealth::VersionUnknown { .. } | AutoloopHealth::Ok { .. } => false,
    }
}

pub fn prompt_accepts(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes")
}

fn response_consents(bytes_read: usize, line: &str) -> bool {
    bytes_read > 0 && prompt_accepts(line)
}

pub fn missing_prompt() -> String {
    format!(
        "autoloop engine not found — download v{VENDORED_AUTOLOOP_VERSION} to ~/.ralph/engine now? [Y/n]"
    )
}

pub fn too_old_prompt(path: &Path, found_version: &str) -> String {
    format!(
        "autoloop engine v{found_version} at {} is too old; the vendored engine will outrank PATH — download v{VENDORED_AUTOLOOP_VERSION} to ~/.ralph/engine now? [Y/n]",
        path.display()
    )
}

pub fn ensure_autoloop_with_provisioning(
    interactive: bool,
    skip_preflight: bool,
) -> Result<AutoloopBin> {
    let health = check_autoloop();

    if !needs_provisioning(&health, skip_preflight) {
        return crate::ensure_autoloop_for_run(health, skip_preflight);
    }

    match provisioning_decision(interactive, auto_install_opt_in()) {
        ProvisionDecision::Refuse => {
            return crate::ensure_autoloop_for_run(health, skip_preflight);
        }
        ProvisionDecision::PromptUser => {
            let prompt = match &health {
                AutoloopHealth::Missing => missing_prompt(),
                AutoloopHealth::TooOld { path, version, .. } => too_old_prompt(path, version),
                _ => unreachable!("only unavailable engines reach the provisioning prompt"),
            };
            eprint!("{prompt} ");
            io::stderr()
                .flush()
                .context("failed to display the autoloop engine download prompt")?;

            let mut response = String::new();
            let bytes_read = io::stdin()
                .read_line(&mut response)
                .context("failed to read the autoloop engine download response")?;
            if !response_consents(bytes_read, &response) {
                return crate::ensure_autoloop_for_run(health, skip_preflight);
            }
        }
        ProvisionDecision::AutoInstall => {}
    }

    crate::engine_install::install().context("failed to provision the autoloop engine")?;
    crate::ensure_autoloop_for_run(check_autoloop(), skip_preflight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::autoloop_health::AutoloopSource;

    #[test]
    fn provisioning_need_respects_health_and_skip_preflight() {
        let too_old = AutoloopHealth::TooOld {
            path: "/usr/local/bin/autoloop".into(),
            version: "0.9.0".into(),
            source: AutoloopSource::PathLookup,
        };
        let ok = AutoloopHealth::Ok {
            path: "/usr/local/bin/autoloop".into(),
            version: VENDORED_AUTOLOOP_VERSION.into(),
            source: AutoloopSource::PathLookup,
        };
        let version_unknown = AutoloopHealth::VersionUnknown {
            path: "/usr/local/bin/autoloop".into(),
            source: AutoloopSource::PathLookup,
        };

        assert!(needs_provisioning(&AutoloopHealth::Missing, false));
        assert!(needs_provisioning(&AutoloopHealth::Missing, true));
        assert!(needs_provisioning(&too_old, false));
        assert!(!needs_provisioning(&too_old, true));
        assert!(!needs_provisioning(&ok, false));
        assert!(!needs_provisioning(&ok, true));
        assert!(!needs_provisioning(&version_unknown, false));
        assert!(!needs_provisioning(&version_unknown, true));
    }

    #[test]
    fn provisioning_decision_respects_interactivity_and_explicit_opt_in() {
        assert_eq!(
            provisioning_decision(true, false),
            ProvisionDecision::PromptUser
        );
        assert_eq!(
            provisioning_decision(false, false),
            ProvisionDecision::Refuse
        );
        assert_eq!(
            provisioning_decision(true, true),
            ProvisionDecision::AutoInstall
        );
        assert_eq!(
            provisioning_decision(false, true),
            ProvisionDecision::AutoInstall
        );
    }

    #[test]
    fn auto_install_opt_in_accepts_only_one_or_true() {
        for accepted in ["1", " 1 ", "true", "TRUE", " True "] {
            assert!(
                auto_install_opt_in_value(Some(accepted)),
                "expected {accepted:?} to opt in"
            );
        }

        for refused in [None, Some(""), Some("0"), Some("yes"), Some("false")] {
            assert!(
                !auto_install_opt_in_value(refused),
                "expected {refused:?} not to opt in"
            );
        }
    }

    #[test]
    fn prompt_acceptance_defaults_to_yes_and_rejects_other_answers() {
        for accepted in ["", " ", "y", "Y", "yes", " YES "] {
            assert!(prompt_accepts(accepted), "expected {accepted:?} to accept");
        }

        for declined in ["n", "N", "no", "garbage"] {
            assert!(
                !prompt_accepts(declined),
                "expected {declined:?} to decline"
            );
        }
    }

    #[test]
    fn prompt_response_requires_input_before_consenting() {
        assert!(!response_consents(0, ""), "EOF must decline");
        assert!(response_consents(1, "\n"), "Enter accepts the default");
        assert!(response_consents(4, "yes\n"));
        assert!(!response_consents(2, "n\n"));
    }

    #[test]
    fn prompts_offer_the_pinned_engine_version() {
        let missing = missing_prompt();
        assert_eq!(
            missing,
            format!(
                "autoloop engine not found — download v{VENDORED_AUTOLOOP_VERSION} to ~/.ralph/engine now? [Y/n]"
            )
        );

        let too_old = too_old_prompt(Path::new("/usr/local/bin/autoloop"), "0.9.0");
        assert!(too_old.contains("v0.9.0"));
        assert!(too_old.contains("/usr/local/bin/autoloop"));
        assert!(too_old.contains(&format!("v{VENDORED_AUTOLOOP_VERSION}")));
        assert!(too_old.contains("outrank PATH"));
    }
}

use ralph_core::autoloop_health::VENDORED_AUTOLOOP_VERSION;
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

pub fn prompt_accepts(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes")
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

#[cfg(test)]
mod tests {
    use super::*;

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

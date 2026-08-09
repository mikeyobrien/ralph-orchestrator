//! Single source of truth for built-in backend identity metadata.
//!
//! `ralph-core` owns the immutable catalog of *named* backends; `ralph-adapters`
//! and `ralph-cli` consume it for auto-detection order, command mapping,
//! validation, help text, and install guidance. Keeping one authority here is
//! what eliminates the historic drift between the adapter priority list, the CLI
//! `VALID_BACKENDS` constant, the preflight command map, and the doctor
//! canonical-name list.
//!
//! `auto` and `custom` are *selectors*, not backends: `auto` defers to
//! [`default_priority`] detection and `custom` carries an arbitrary command, so
//! neither belongs in the catalog of built-in identities.

/// Immutable identity metadata for a built-in backend.
///
/// All fields are `'static` so a [`BackendMetadata`] is [`Copy`] and cheap to
/// pass around. The catalog is the authority for these four facets; per-backend
/// *behavior* (argv, output format, auth env vars) continues to live in
/// `ralph-adapters` and is intentionally not modeled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendMetadata {
    /// Stable config/CLI identifier, e.g. `"claude"` or `"kiro-acp"`.
    pub id: &'static str,
    /// Human-readable name for help text and error messages.
    pub display_name: &'static str,
    /// Executable invoked for this backend. May differ from `id` — e.g. both
    /// `kiro` and `kiro-acp` resolve to the `kiro-cli` binary.
    pub command: &'static str,
    /// Official install / documentation link rendered in no-backend output.
    pub install_url: &'static str,
}

/// The catalog of built-in backends in canonical auto-detection order.
///
/// Order exactly preserves the historic `auto_detect::DEFAULT_PRIORITY`,
/// including Roo, with OMP appended last (the newest named backend) so existing
/// installations keep selecting the same backend. New named backends are
/// appended here (and only here) to appear in detection, validation, help, and
/// install guidance simultaneously.
static CATALOG: &[BackendMetadata] = &[
    BackendMetadata {
        id: "claude",
        display_name: "Claude",
        command: "claude",
        install_url: "https://docs.anthropic.com/claude-code",
    },
    BackendMetadata {
        id: "kiro",
        display_name: "Kiro",
        command: "kiro-cli",
        install_url: "https://kiro.dev",
    },
    BackendMetadata {
        id: "kiro-acp",
        display_name: "Kiro ACP",
        command: "kiro-cli",
        install_url: "https://kiro.dev",
    },
    BackendMetadata {
        id: "gemini",
        display_name: "Gemini",
        command: "gemini",
        install_url: "https://cloud.google.com/gemini",
    },
    BackendMetadata {
        id: "codex",
        display_name: "Codex",
        command: "codex",
        install_url: "https://openai.com/codex",
    },
    BackendMetadata {
        id: "forge",
        display_name: "Forge",
        command: "forge",
        install_url: "https://github.com/tailcallhq/forgecode",
    },
    BackendMetadata {
        id: "amp",
        display_name: "Amp",
        command: "amp",
        install_url: "https://amp.dev",
    },
    BackendMetadata {
        id: "copilot",
        display_name: "Copilot",
        command: "copilot",
        install_url: "https://docs.github.com/copilot",
    },
    BackendMetadata {
        id: "opencode",
        display_name: "OpenCode",
        command: "opencode",
        install_url: "https://opencode.ai",
    },
    BackendMetadata {
        id: "pi",
        display_name: "Pi",
        command: "pi",
        install_url: "https://www.npmjs.com/package/@mariozechner/pi-coding-agent",
    },
    BackendMetadata {
        id: "roo",
        display_name: "Roo",
        command: "roo",
        install_url: "https://github.com/RooVetGit/Roo-Code",
    },
    BackendMetadata {
        id: "omp",
        display_name: "OMP",
        command: "omp",
        install_url: "https://github.com/can1357/oh-my-pi",
    },
];

/// Iterates every catalogued backend in canonical (auto-detection) order.
pub fn iter() -> impl DoubleEndedIterator<Item = BackendMetadata> + Clone {
    CATALOG.iter().copied()
}

/// Looks up a backend by its identifier.
pub fn lookup(name: &str) -> Option<BackendMetadata> {
    CATALOG.iter().copied().find(|metadata| metadata.id == name)
}

/// Returns `true` if `name` is a catalogued (named, built-in) backend.
///
/// `auto` and `custom` are selectors, not named backends, so they return
/// `false` here. Use [`default_priority`] / [`valid_names`] for the set of
/// acceptable names in validation messages.
pub fn is_named(name: &str) -> bool {
    lookup(name).is_some()
}

/// The executable command for a backend, if it is catalogued.
///
/// Both `kiro` and `kiro-acp` map to `kiro-cli`; every other built-in backend
/// uses its own name as the command.
pub fn command_for(name: &str) -> Option<&'static str> {
    lookup(name).map(|metadata| metadata.command)
}

/// The official install URL for a backend, if it is catalogued.
pub fn install_url_for(name: &str) -> Option<&'static str> {
    lookup(name).map(|metadata| metadata.install_url)
}

/// The canonical auto-detection priority order — every catalogued backend id.
///
/// Consumers should treat this as the single source for detection order; a
/// disabled backend is skipped only during auto-detection, never when the user
/// selects it explicitly.
pub fn default_priority() -> Vec<&'static str> {
    CATALOG.iter().map(|metadata| metadata.id).collect()
}

/// Every acceptable named-backend identifier, in canonical order.
///
/// This is the set used by validation error messages ("valid backends: …").
/// It is identical to [`default_priority`] because every catalogued backend is
/// both detectable and selectable.
pub fn valid_names() -> Vec<&'static str> {
    default_priority()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The exact canonical order the catalog must preserve (historic
    /// `auto_detect::DEFAULT_PRIORITY`, including Roo, with OMP appended last).
    const EXPECTED_ORDER: &[&str] = &[
        "claude", "kiro", "kiro-acp", "gemini", "codex", "forge", "amp", "copilot", "opencode",
        "pi", "roo", "omp",
    ];

    #[test]
    fn catalog_ids_match_expected_order_exactly() {
        let ids: Vec<&str> = iter().map(|m| m.id).collect();
        assert_eq!(
            ids, EXPECTED_ORDER,
            "catalog order must preserve the historic detection priority exactly"
        );
    }

    #[test]
    fn default_priority_matches_catalog_order() {
        assert_eq!(default_priority(), EXPECTED_ORDER);
    }

    #[test]
    fn valid_names_matches_catalog_order() {
        assert_eq!(valid_names(), EXPECTED_ORDER);
    }

    #[test]
    fn catalog_ids_are_unique() {
        let ids: Vec<&str> = iter().map(|m| m.id).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "backend ids must be unique");
    }

    #[test]
    fn omp_is_present_and_last() {
        // OMP is the newest named backend, appended after Roo so existing
        // installations keep their detection order. Roo remains present.
        let ids: Vec<&str> = iter().map(|m| m.id).collect();
        assert!(ids.contains(&"roo"), "roo must remain in the catalog");
        assert_eq!(
            ids.last().copied(),
            Some("omp"),
            "omp must be appended last"
        );
    }

    #[test]
    fn tail_order_is_pi_then_roo_then_omp() {
        // The three trailing entries are stable: pi, roo, omp. Guards against a
        // future append that reorders the tail.
        let ids: Vec<&str> = iter().map(|m| m.id).collect();
        let len = ids.len();
        assert_eq!(&ids[len - 3..], &["pi", "roo", "omp"]);
    }

    #[test]
    fn auto_and_custom_are_not_catalog_entries() {
        assert!(!is_named("auto"));
        assert!(!is_named("custom"));
        assert!(lookup("auto").is_none());
        assert!(lookup("custom").is_none());
        assert!(!valid_names().contains(&"auto"));
        assert!(!valid_names().contains(&"custom"));
    }

    #[test]
    fn kiro_and_kiro_acp_map_to_kiro_cli_command() {
        assert_eq!(command_for("kiro"), Some("kiro-cli"));
        assert_eq!(command_for("kiro-acp"), Some("kiro-cli"));
    }

    #[test]
    fn named_backends_use_own_command_except_kiro_family() {
        for metadata in iter() {
            let expected = if metadata.id == "kiro" || metadata.id == "kiro-acp" {
                "kiro-cli"
            } else {
                metadata.id
            };
            assert_eq!(
                metadata.command, expected,
                "command mismatch for backend {}",
                metadata.id
            );
            // The command_for helper must agree with the metadata.
            assert_eq!(command_for(metadata.id), Some(expected));
        }
    }

    #[test]
    fn lookup_returns_known_and_rejects_unknown() {
        assert_eq!(lookup("claude").map(|m| m.id), Some("claude"));
        assert_eq!(lookup("roo").map(|m| m.id), Some("roo"));
        assert!(lookup("ompp").is_none());
        assert!(lookup("").is_none());
        assert!(lookup("CLAUDE").is_none(), "lookup is case-sensitive");
    }

    #[test]
    fn is_named_covers_all_catalogued_and_rejects_rest() {
        for metadata in iter() {
            assert!(is_named(metadata.id));
        }
        assert!(!is_named("ompp"));
        assert!(!is_named(""));
    }

    #[test]
    fn every_entry_has_nonempty_metadata() {
        for metadata in iter() {
            assert!(!metadata.id.is_empty(), "empty id");
            assert!(
                !metadata.display_name.is_empty(),
                "empty display_name for {}",
                metadata.id
            );
            assert!(
                !metadata.command.is_empty(),
                "empty command for {}",
                metadata.id
            );
            assert!(
                !metadata.install_url.is_empty(),
                "empty install_url for {}",
                metadata.id
            );
            assert!(
                metadata.install_url.starts_with("https://"),
                "install_url for {} must be https: {}",
                metadata.id,
                metadata.install_url
            );
        }
    }

    #[test]
    fn install_url_helper_agrees_with_metadata() {
        for metadata in iter() {
            assert_eq!(install_url_for(metadata.id), Some(metadata.install_url));
        }
        assert!(install_url_for("ompp").is_none());
    }

    #[test]
    fn iter_is_reversible_and_cloneable() {
        // OMP first when reversed (it is last forward) — guards the
        // DoubleEndedIterator + Clone bound.
        let reversed: Vec<&str> = iter().rev().map(|m| m.id).collect();
        assert_eq!(reversed.first().copied(), Some("omp"));

        let cloned: Vec<&str> = iter().clone().map(|m| m.id).collect();
        assert_eq!(cloned, EXPECTED_ORDER);
    }
}

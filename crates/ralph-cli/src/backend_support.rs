//! Shared backend metadata for CLI validation and user-facing error messages.
//!
//! All accepted backend identifiers are derived from the [`ralph_core::backend`]
//! catalog so a new backend appears in every CLI surface (validation, help,
//! init, hat/SOP selection) from a single source. The `custom` selector carries
//! an arbitrary command and is accepted alongside catalogued backends; `auto`
//! is a detection selector, not a selectable backend, so it is intentionally
//! excluded.

use ralph_core::backend;

/// Every backend identifier accepted by CLI selectors.
///
/// Catalogued backends (in canonical order) plus the `custom` selector. `auto`
/// is excluded because it defers to detection rather than naming a backend.
pub fn valid_backend_names() -> Vec<&'static str> {
    let mut names = backend::valid_names();
    names.push("custom");
    names
}

/// Human-readable list of accepted backends for CLI messages and docs.
pub fn valid_backend_label() -> String {
    valid_backend_names().join(", ")
}

/// Returns `true` if the backend identifier is known (catalogued or `custom`).
pub fn is_known_backend(name: &str) -> bool {
    name == "custom" || backend::is_named(name)
}

/// Formats the canonical unknown-backend error with all supported backends.
pub fn unknown_backend_message(name: &str) -> String {
    format!(
        "Unknown backend: {}\n\nValid backends: {}",
        name,
        valid_backend_label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_adapters::{CliBackend, default_priority};

    #[test]
    fn every_catalog_backend_is_accepted_across_all_consumer_surfaces() {
        // Consolidated catalog↔consumer conformance (TR8): for every catalogued
        // backend, the factory constructs it with a command that agrees with the
        // catalog, the interactive factory accepts it, every CLI validation/help
        // surface accepts it, and it appears in the detection priority. A backend
        // added to the catalog without a matching factory/validation/detection
        // entry fails here. `roo` in particular was previously rejected by
        // validation because a parallel VALID_BACKENDS list omitted it.
        let priority = default_priority();
        for metadata in ralph_core::backend::iter() {
            let headless = CliBackend::from_name(metadata.id)
                .unwrap_or_else(|e| panic!("from_name({}) should succeed: {e}", metadata.id));
            assert_eq!(
                headless.command, metadata.command,
                "from_name command drift for {}",
                metadata.id
            );
            assert!(
                CliBackend::for_interactive_prompt(metadata.id).is_ok(),
                "for_interactive_prompt should accept {}",
                metadata.id
            );
            assert!(
                is_known_backend(metadata.id),
                "is_known_backend should accept {}",
                metadata.id
            );
            let names = valid_backend_names();
            assert!(
                names.contains(&metadata.id),
                "valid_backend_names missing {}",
                metadata.id
            );
            assert!(
                priority.contains(&metadata.id),
                "default_priority missing {}",
                metadata.id
            );
        }
        // Selectors: `custom` is a valid CLI backend; `auto` is detection-only.
        assert!(is_known_backend("custom"));
        assert!(valid_backend_names().contains(&"custom"));
        assert!(!is_known_backend("auto"));
        assert!(!valid_backend_names().contains(&"auto"));
        assert!(!is_known_backend("ompp"));
    }

    #[test]
    fn valid_backend_label_lists_every_catalog_name() {
        let label = valid_backend_label();
        for metadata in ralph_core::backend::iter() {
            assert!(label.contains(metadata.id), "label missing {}", metadata.id);
        }
        assert!(label.contains("custom"));
    }
}

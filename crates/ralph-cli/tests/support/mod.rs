//! Shared helpers for the ralph CLI integration tests.

use std::path::Path;
use std::sync::OnceLock;

use tempfile::TempDir;

/// One empty `HOME` for this test binary.
///
/// Ralph merges `~/.ralph/config.yml` into every run, so a test that exercises
/// defaults must not inherit the operator's home directory. The temp dir lives
/// for the whole binary, which matches the clean environment CI provides.
pub fn isolated_home() -> &'static Path {
    static HOME: OnceLock<TempDir> = OnceLock::new();
    HOME.get_or_init(|| TempDir::new().expect("isolated home"))
        .path()
}

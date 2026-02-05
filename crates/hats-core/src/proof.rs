//! Proof artifact generation for BDD dark factory workflow.
//!
//! Proof artifacts are JSON files that record the outcome of a BDD loop:
//! what was specified, what was implemented, what was tested, and whether
//! it passed. They live in `.hats/proofs/<loop-id>.json`.
//!
//! # Schema
//!
//! ```json
//! {
//!   "spec_file": "features/auth.feature",
//!   "scenarios_total": 4,
//!   "tests_pass": 4,
//!   "tests_fail": 0,
//!   "iterations": 2,
//!   "duration_secs": 45.2,
//!   "files_changed": ["src/auth.rs", "tests/auth_test.rs"],
//!   "git_sha": "abc123def456",
//!   "exit_code": 0
//! }
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use hats_core::proof::{ProofArtifact, write_proof};
//! use std::path::Path;
//!
//! let proof = ProofArtifact {
//!     spec_file: "features/auth.feature".to_string(),
//!     scenarios_total: 4,
//!     tests_pass: 4,
//!     tests_fail: 0,
//!     iterations: 2,
//!     duration_secs: 45.2,
//!     files_changed: vec!["src/auth.rs".to_string()],
//!     git_sha: "abc123".to_string(),
//!     exit_code: 0,
//! };
//!
//! write_proof(Path::new(".hats"), "loop-20260205-a1b2", &proof).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// A proof artifact recording the outcome of a BDD loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofArtifact {
    /// Path to the input feature file.
    pub spec_file: String,

    /// Number of scenarios in the feature file.
    pub scenarios_total: u32,

    /// Number of passing acceptance tests.
    pub tests_pass: u32,

    /// Number of failing acceptance tests.
    pub tests_fail: u32,

    /// Number of loop iterations executed.
    pub iterations: u32,

    /// Wall-clock seconds from start to finish.
    pub duration_secs: f64,

    /// List of files created or modified.
    pub files_changed: Vec<String>,

    /// Commit SHA at loop completion.
    pub git_sha: String,

    /// 0 for success, non-zero for failure.
    pub exit_code: i32,
}

impl ProofArtifact {
    /// Returns true if all tests passed (exit_code == 0).
    pub fn is_success(&self) -> bool {
        self.exit_code == 0 && self.tests_fail == 0
    }

    /// Returns a summary string for logging.
    pub fn summary(&self) -> String {
        format!(
            "{}: {}/{} scenarios pass, {} iterations, {:.1}s",
            self.spec_file,
            self.tests_pass,
            self.scenarios_total,
            self.iterations,
            self.duration_secs,
        )
    }
}

/// Returns the proofs directory path within a .hats directory.
pub fn proofs_dir(hats_dir: &Path) -> PathBuf {
    hats_dir.join("proofs")
}

/// Returns the path for a specific proof file.
pub fn proof_path(hats_dir: &Path, loop_id: &str) -> PathBuf {
    proofs_dir(hats_dir).join(format!("{loop_id}.json"))
}

/// Writes a proof artifact to `.hats/proofs/<loop-id>.json`.
///
/// Creates the proofs directory if it doesn't exist.
///
/// # Errors
///
/// Returns an error if the directory can't be created or the file can't be written.
pub fn write_proof(
    hats_dir: &Path,
    loop_id: &str,
    proof: &ProofArtifact,
) -> anyhow::Result<PathBuf> {
    let dir = proofs_dir(hats_dir);
    fs::create_dir_all(&dir)?;

    let path = proof_path(hats_dir, loop_id);
    let json = serde_json::to_string_pretty(proof)?;
    fs::write(&path, &json)?;

    info!("Proof artifact written to {}", path.display());
    debug!("Proof: {}", proof.summary());

    Ok(path)
}

/// Reads a proof artifact from `.hats/proofs/<loop-id>.json`.
///
/// # Errors
///
/// Returns an error if the file doesn't exist or contains invalid JSON.
pub fn read_proof(hats_dir: &Path, loop_id: &str) -> anyhow::Result<ProofArtifact> {
    let path = proof_path(hats_dir, loop_id);
    let json = fs::read_to_string(&path)?;
    let proof: ProofArtifact = serde_json::from_str(&json)?;
    Ok(proof)
}

/// Lists all proof artifacts in the proofs directory.
///
/// Returns a vector of (loop_id, ProofArtifact) tuples.
pub fn list_proofs(hats_dir: &Path) -> anyhow::Result<Vec<(String, ProofArtifact)>> {
    let dir = proofs_dir(hats_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut proofs = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            let loop_id = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match fs::read_to_string(&path)
                .ok()
                .and_then(|json| serde_json::from_str::<ProofArtifact>(&json).ok())
            {
                Some(proof) => proofs.push((loop_id, proof)),
                None => {
                    debug!("Skipping invalid proof file: {}", path.display());
                }
            }
        }
    }

    Ok(proofs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_proof() -> ProofArtifact {
        ProofArtifact {
            spec_file: "features/auth.feature".to_string(),
            scenarios_total: 4,
            tests_pass: 4,
            tests_fail: 0,
            iterations: 2,
            duration_secs: 45.2,
            files_changed: vec![
                "src/auth.rs".to_string(),
                "tests/auth_test.rs".to_string(),
            ],
            git_sha: "abc123def456".to_string(),
            exit_code: 0,
        }
    }

    #[test]
    fn test_write_and_read_proof() {
        let tmp = TempDir::new().unwrap();
        let hats_dir = tmp.path().join(".hats");

        let proof = sample_proof();
        let path = write_proof(&hats_dir, "loop-001", &proof).unwrap();

        assert!(path.exists());
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), "loop-001.json");

        let read_back = read_proof(&hats_dir, "loop-001").unwrap();
        assert_eq!(read_back, proof);
    }

    #[test]
    fn test_proof_is_valid_json() {
        let tmp = TempDir::new().unwrap();
        let hats_dir = tmp.path().join(".hats");

        let proof = sample_proof();
        write_proof(&hats_dir, "loop-002", &proof).unwrap();

        let path = proof_path(&hats_dir, "loop-002");
        let json = fs::read_to_string(path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["spec_file"], "features/auth.feature");
        assert_eq!(value["scenarios_total"], 4);
        assert_eq!(value["tests_pass"], 4);
        assert_eq!(value["tests_fail"], 0);
        assert_eq!(value["iterations"], 2);
        assert_eq!(value["exit_code"], 0);
        assert!(value["files_changed"].is_array());
        assert!(value["git_sha"].is_string());
        assert!(value["duration_secs"].is_number());
    }

    #[test]
    fn test_proof_is_success() {
        let mut proof = sample_proof();
        assert!(proof.is_success());

        proof.tests_fail = 1;
        assert!(!proof.is_success());

        proof.tests_fail = 0;
        proof.exit_code = 1;
        assert!(!proof.is_success());
    }

    #[test]
    fn test_proof_summary() {
        let proof = sample_proof();
        let summary = proof.summary();
        assert!(summary.contains("features/auth.feature"));
        assert!(summary.contains("4/4"));
        assert!(summary.contains("2 iterations"));
    }

    #[test]
    fn test_proof_failure_state() {
        let tmp = TempDir::new().unwrap();
        let hats_dir = tmp.path().join(".hats");

        let proof = ProofArtifact {
            spec_file: "features/cart.feature".to_string(),
            scenarios_total: 3,
            tests_pass: 1,
            tests_fail: 2,
            iterations: 5,
            duration_secs: 120.5,
            files_changed: vec!["src/cart.rs".to_string()],
            git_sha: "def789".to_string(),
            exit_code: 1,
        };

        write_proof(&hats_dir, "loop-fail", &proof).unwrap();
        let read_back = read_proof(&hats_dir, "loop-fail").unwrap();

        assert!(!read_back.is_success());
        assert!(read_back.tests_fail > 0);
        assert_ne!(read_back.exit_code, 0);
    }

    #[test]
    fn test_read_nonexistent_proof() {
        let tmp = TempDir::new().unwrap();
        let hats_dir = tmp.path().join(".hats");
        let result = read_proof(&hats_dir, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_proofs_empty() {
        let tmp = TempDir::new().unwrap();
        let hats_dir = tmp.path().join(".hats");
        let proofs = list_proofs(&hats_dir).unwrap();
        assert!(proofs.is_empty());
    }

    #[test]
    fn test_list_proofs_multiple() {
        let tmp = TempDir::new().unwrap();
        let hats_dir = tmp.path().join(".hats");

        let proof1 = sample_proof();
        let mut proof2 = sample_proof();
        proof2.spec_file = "features/cart.feature".to_string();

        write_proof(&hats_dir, "loop-a", &proof1).unwrap();
        write_proof(&hats_dir, "loop-b", &proof2).unwrap();

        let proofs = list_proofs(&hats_dir).unwrap();
        assert_eq!(proofs.len(), 2);
    }

    #[test]
    fn test_proofs_dir_path() {
        let hats_dir = Path::new("/project/.hats");
        assert_eq!(
            proofs_dir(hats_dir).to_string_lossy(),
            "/project/.hats/proofs"
        );
    }

    #[test]
    fn test_proof_path_format() {
        let hats_dir = Path::new("/project/.hats");
        assert_eq!(
            proof_path(hats_dir, "loop-20260205-a1b2").to_string_lossy(),
            "/project/.hats/proofs/loop-20260205-a1b2.json"
        );
    }
}

//! PRP file discovery and parsing utilities.
//!
//! This module handles:
//! - Scanning directories for PRP markdown files
//! - Extracting PRP ID and title from filenames
//! - Parsing markdown to extract title and Definition of Done status

use std::fs;
use std::path::{Path, PathBuf};

/// Discovered PRP file information.
#[derive(Debug, Clone)]
pub struct DiscoveredPrp {
    /// PRP ID extracted from filename (e.g., "PRP-001").
    pub prp_id: String,

    /// Full path to the PRP markdown file.
    pub path: PathBuf,

    /// Title extracted from the markdown (first H1 heading).
    pub title: String,
}

/// Errors that can occur during PRP file operations.
#[derive(Debug, thiserror::Error)]
pub enum PrpFilesError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid filename format.
    #[error("Invalid PRP filename: {0}")]
    InvalidFilename(String),
}

/// Parses a PRP ID from a path.
///
/// Extracts the stem from a filename like "PRP-001.md" and validates it starts with "PRP-".
///
/// # Arguments
///
/// * `path` - Path to the PRP markdown file
///
/// # Returns
///
/// The PRP ID (e.g., "PRP-001") or an error if invalid
pub fn parse_prp_id(path: &Path) -> Result<String, PrpFilesError> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| PrpFilesError::InvalidFilename(format!("No file stem: {:?}", path)))?;

    if !stem.starts_with("PRP-") {
        return Err(PrpFilesError::InvalidFilename(format!(
            "Filename does not start with PRP-: {}",
            stem
        )));
    }

    Ok(stem.to_string())
}

/// Extracts the title from a PRP markdown file.
///
/// Looks for the first H1 heading (# Title) and returns its text.
/// If no H1 is found, returns the PRP ID as the title.
///
/// # Arguments
///
/// * `path` - Path to the PRP markdown file
///
/// # Returns
///
/// The extracted title or PRP ID as fallback
pub fn extract_title(path: &Path) -> Result<String, PrpFilesError> {
    let content = fs::read_to_string(path)?;

    // Look for first H1 heading: # Title
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Ok(trimmed[2..].trim().to_string());
        }
    }

    // Fallback to PRP ID from filename
    parse_prp_id(path)
}

/// Checks if all Definition of Done checkboxes are checked.
///
/// Looks for a "## Definition of Done" section and checks if all
/// checkbox items are marked as done ([x] instead of [ ]).
///
/// # Arguments
///
/// * `path` - Path to the PRP markdown file
///
/// # Returns
///
/// true if DoD section exists and all items are checked, false otherwise
pub fn definition_of_done_complete(path: &Path) -> Result<bool, PrpFilesError> {
    let content = fs::read_to_string(path)?;

    let mut in_dod_section = false;
    let mut found_dod_item = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for DoD section header
        if trimmed == "## Definition of Done" || trimmed == "## Definition of Done" {
            in_dod_section = true;
            continue;
        }

        // Check for next section (any ## heading stops DoD parsing)
        if trimmed.starts_with("## ") {
            break;
        }

        if in_dod_section {
            // Look for checkbox items: - [x] or - [X]
            if trimmed.starts_with("- [") {
                found_dod_item = true;
                // Check if it's NOT checked (lowercase x is unchecked)
                if trimmed.starts_with("- [ ]") {
                    // Found unchecked item - DoD not complete
                    return Ok(false);
                }
            }
        }
    }

    // If we found DoD items, all must be checked (already returned false if any unchecked)
    // If we found no DoD items, treat as incomplete
    Ok(found_dod_item)
}

/// Discovers all PRP markdown files in a directory.
///
/// Scans the given directory for files matching "PRP-*.md" pattern.
///
/// # Arguments
///
/// * `import_dir` - Directory to scan for PRP files
///
/// # Returns
///
/// List of discovered PRP files with their IDs and titles
pub fn discover_prps(import_dir: &Path) -> Result<Vec<DiscoveredPrp>, PrpFilesError> {
    if !import_dir.exists() {
        return Ok(Vec::new());
    }

    let mut prps = Vec::new();

    for entry in fs::read_dir(import_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .md files
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        // Try to parse PRP ID
        let prp_id = match parse_prp_id(&path) {
            Ok(id) => id,
            Err(_) => continue, // Skip non-PRP files
        };

        // Extract title
        let title = extract_title(&path).unwrap_or_else(|_| prp_id.clone());

        prps.push(DiscoveredPrp {
            prp_id,
            path,
            title,
        });
    }

    // Sort by PRP ID for consistent ordering
    prps.sort_by(|a, b| a.prp_id.cmp(&b.prp_id));

    Ok(prps)
}

/// Archives a PRP by moving it from remaining_work to completed.
///
/// # Arguments
///
/// * `source_path` - Current path to the PRP markdown
/// * `completed_dir` - Directory where completed PRPs are stored
///
/// # Returns
///
/// The path to the archived file
pub fn archive_prp(
    source_path: &Path,
    completed_dir: &Path,
) -> Result<PathBuf, PrpFilesError> {
    let filename = source_path
        .file_name()
        .ok_or_else(|| PrpFilesError::InvalidFilename(format!("No filename: {:?}", source_path)))?;

    // Ensure completed directory exists
    fs::create_dir_all(completed_dir)?;

    let dest_path = completed_dir.join(filename);

    // Move the file
    fs::rename(source_path, &dest_path)?;

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_prp_id_valid() {
        let result = parse_prp_id(Path::new("PRPs/remaining_work/PRP-001.md")).unwrap();
        assert_eq!(result, "PRP-001");
    }

    #[test]
    fn test_parse_prp_id_invalid() {
        let result = parse_prp_id(Path::new("PRPs/remaining_work/other-file.md"));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_title() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("PRP-001.md");
        fs::write(
            &path,
            r#"# My Test PRP

## Status
- [ ] In Progress

## Definition of Done
- [x] Done
"#,
        )
        .unwrap();

        let title = extract_title(&path).unwrap();
        assert_eq!(title, "My Test PRP");
    }

    #[test]
    fn test_extract_title_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("PRP-001.md");
        fs::write(&path, "No title here").unwrap();

        let title = extract_title(&path).unwrap();
        assert_eq!(title, "PRP-001");
    }

    #[test]
    fn test_dod_complete_true() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("PRP-001.md");
        fs::write(
            &path,
            r#"# Test

## Definition of Done
- [x] Item 1
- [x] Item 2
"#,
        )
        .unwrap();

        assert!(definition_of_done_complete(&path).unwrap());
    }

    #[test]
    fn test_dod_complete_false() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("PRP-001.md");
        fs::write(
            &path,
            r#"# Test

## Definition of Done
- [x] Item 1
- [ ] Item 2
"#,
        )
        .unwrap();

        assert!(!definition_of_done_complete(&path).unwrap());
    }

    #[test]
    fn test_dod_complete_no_items() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("PRP-001.md");
        fs::write(
            &path,
            r#"# Test

## Definition of Done
No items here
"#,
        )
        .unwrap();

        // No checkbox items means not complete
        assert!(!definition_of_done_complete(&path).unwrap());
    }

    #[test]
    fn test_discover_prps() {
        let temp_dir = TempDir::new().unwrap();
        let prps_dir = temp_dir.path().join("PRPs/remaining_work");
        fs::create_dir_all(&prps_dir).unwrap();

        fs::write(prps_dir.join("PRP-002.md"), "# Second").unwrap();
        fs::write(prps_dir.join("PRP-001.md"), "# First").unwrap();
        fs::write(prps_dir.join("README.md"), "# README").unwrap(); // Should be skipped

        let prps = discover_prps(&prps_dir).unwrap();

        assert_eq!(prps.len(), 2);
        assert_eq!(prps[0].prp_id, "PRP-001");
        assert_eq!(prps[1].prp_id, "PRP-002");
    }

    #[test]
    fn test_archive_prp() {
        let temp_dir = TempDir::new().unwrap();
        let remaining = temp_dir.path().join("remaining_work");
        let completed = temp_dir.path().join("completed");
        fs::create_dir_all(&remaining).unwrap();
        fs::create_dir_all(&completed).unwrap();

        let source = remaining.join("PRP-001.md");
        fs::write(&source, "# Test").unwrap();

        let archived = archive_prp(&source, &completed).unwrap();

        assert!(!source.exists());
        assert!(archived.exists());
        assert_eq!(archived.file_name().unwrap(), "PRP-001.md");
    }
}

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let repository_root = manifest_dir.join("../..");
    let package_version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");

    let is_repository_root = is_git_repository_root(&repository_root);
    if is_repository_root {
        watch_git_head(&repository_root);
    }

    // Keep release-tagged binaries aligned with their published package version. Unreleased
    // source builds include the commit hash so they can be distinguished from that release.
    let git_hash =
        if !is_repository_root || is_matching_release_tag(&repository_root, &package_version) {
            None
        } else {
            git_output(&repository_root, &["rev-parse", "--short=7", "HEAD"])
        };

    let version = git_hash.as_ref().map_or_else(
        || package_version.clone(),
        |hash| format!("{package_version}+{hash}"),
    );

    println!("cargo:rustc-env=RALPH_VERSION={version}");
    println!(
        "cargo:rustc-env=RALPH_GIT_HASH={}",
        git_hash.unwrap_or_default()
    );
}

fn is_git_repository_root(repository_root: &Path) -> bool {
    let Some(git_root) = git_output(repository_root, &["rev-parse", "--show-toplevel"]) else {
        return false;
    };
    let Ok(git_root) = PathBuf::from(git_root).canonicalize() else {
        return false;
    };
    let Ok(repository_root) = repository_root.canonicalize() else {
        return false;
    };

    git_root == repository_root
}

fn is_matching_release_tag(repository_root: &Path, package_version: &str) -> bool {
    git_output(
        repository_root,
        &["describe", "--exact-match", "--tags", "HEAD"],
    )
    .is_some_and(|tag| tag.strip_prefix('v').unwrap_or(&tag) == package_version)
}

fn watch_git_head(repository_root: &Path) {
    let Some(head_path) = git_path(repository_root, "HEAD") else {
        return;
    };

    println!("cargo:rerun-if-changed={}", head_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        git_path(repository_root, "packed-refs")
            .unwrap_or_else(|| repository_root.join(".git/packed-refs"))
            .display()
    );

    if let Ok(head) = fs::read_to_string(&head_path)
        && let Some(reference) = head.trim().strip_prefix("ref: ")
        && let Some(reference_path) = git_path(repository_root, reference)
    {
        println!("cargo:rerun-if-changed={}", reference_path.display());
    }
}

fn git_path(repository_root: &Path, path: &str) -> Option<PathBuf> {
    let path = git_output(repository_root, &["rev-parse", "--git-path", path])?;
    let path = PathBuf::from(path);
    Some(if path.is_absolute() {
        path
    } else {
        repository_root.join(path)
    })
}

fn git_output(repository_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

use std::process::Command;

#[test]
fn version_reports_the_build_version_and_git_hash() {
    let package_version = env!("CARGO_PKG_VERSION");
    let git_hash = env!("RALPH_GIT_HASH");
    let expected_version = if git_hash.is_empty() {
        package_version.to_owned()
    } else {
        format!("{package_version}+{git_hash}")
    };

    assert_eq!(env!("RALPH_VERSION"), expected_version);

    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("--version")
        .output()
        .expect("run ralph --version");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("ralph {expected_version}")
    );
}

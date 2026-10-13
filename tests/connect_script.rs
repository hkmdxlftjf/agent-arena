//! shellcheck plus a dry-run of the onboarding script.

#[test]
fn connect_script_shellcheck_and_dry_run() {
    let status = std::process::Command::new("sh")
        .arg("scripts/test-connect.sh")
        .status()
        .expect("run scripts/test-connect.sh");
    assert!(status.success(), "connect script test failed");
}

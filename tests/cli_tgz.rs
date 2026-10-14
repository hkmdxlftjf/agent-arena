//! The server tarball is an npm package `npx` can run without a registry.

use std::process::Command;

#[test]
fn cli_tarball_runs_a_dry_connect() {
    let bytes = arena::http::cli_tarball();
    assert!(bytes.starts_with(&[0x1f, 0x8b]), "expected gzip");
    assert!(bytes.len() < 200_000, "tarball is {} bytes", bytes.len());

    let dir = std::env::temp_dir().join(format!("arena-cli-tgz-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tgz = dir.join("cli.tgz");
    std::fs::write(&tgz, bytes).unwrap();
    let listed = Command::new("tar")
        .args(["-tzf"])
        .arg(&tgz)
        .output()
        .unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let names = String::from_utf8(listed.stdout).unwrap();
    for needle in [
        "package/package.json",
        "package/packages/cli/bin.mjs",
        "package/packages/cli/lib/connect.mjs",
        "package/packages/cli/lib/write.cjs",
    ] {
        assert!(names.contains(needle), "{names}");
    }
    let extracted = Command::new("tar")
        .args(["-xzf"])
        .arg(&tgz)
        .arg("-C")
        .arg(&dir)
        .status()
        .unwrap();
    assert!(extracted.success());

    let home = dir.join("home");
    std::fs::create_dir_all(home.join(".cursor")).unwrap();
    let token = "cli-tarball-secret-token";
    let run = Command::new("node")
        .arg(dir.join("package/packages/cli/bin.mjs"))
        .args([
            "connect",
            "--dry-run",
            "--yes",
            "--token",
            token,
            "--home",
            home.to_str().unwrap(),
            "--base-url",
            "https://agent.idou.bid",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "{stderr}\n{stdout}");
    assert!(stdout.contains("dry-run: 1"), "{stdout}");
    assert!(
        !stdout.contains(token) && !stderr.contains(token),
        "{stdout}\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

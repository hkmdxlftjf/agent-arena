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
        "package/packages/cli/lib/gateway.mjs",
        "package/packages/cli/lib/gateway-remote.mjs",
        "package/packages/cli/lib/gateway-store.mjs",
        "package/skills/arena/scripts/sign.mjs",
        "package/skills/arena/scripts/sign-lock.mjs",
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
    let signer = Command::new("node")
        .arg(dir.join("package/skills/arena/scripts/sign.mjs"))
        .arg("version")
        .env("ARENA_HOME", &home)
        .output()
        .unwrap();
    assert!(
        signer.status.success(),
        "{}",
        String::from_utf8_lossy(&signer.stderr)
    );
    let local = Command::new("node")
        .arg(dir.join("package/packages/cli/bin.mjs"))
        .args([
            "connect",
            "--local",
            "--skip-network",
            "--clients",
            "cursor",
            "--home",
        ])
        .arg(&home)
        .env("ARENA_TOKEN", token)
        .env("ARENA_HOME", home.join("agent"))
        .env("ARENA_SKILL_SRC", dir.join("package/skills/arena"))
        .output()
        .unwrap();
    assert!(
        local.status.success(),
        "{}",
        String::from_utf8_lossy(&local.stderr)
    );
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.join(".cursor/mcp.json")).unwrap()).unwrap();
    let entry = &config["mcpServers"]["agent-arena"];
    assert_eq!(
        entry["args"][0],
        home.join("agent/runtime/bin.mjs").to_str().unwrap()
    );
    assert_eq!(entry["args"][1], "serve");
    assert_eq!(entry["env"]["ARENA_TOKEN"], token);
    // Remove the extracted npx package; the client must use its persistent copy.
    std::fs::remove_dir_all(dir.join("package")).unwrap();
    let mut child = Command::new(entry["command"].as_str().unwrap())
        .args(
            entry["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap()),
        )
        .envs(
            entry["env"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k, v.as_str().unwrap())),
        )
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let initialized: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        initialized["result"]["serverInfo"]["name"],
        "agent-arena-local"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[sqlx::test]
async fn hosted_skill_zip_executes_signer_and_contains_gateway(pool: sqlx::PgPool) {
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    let dir = std::env::temp_dir().join(format!("arena-skill-zip-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let app = arena::app::App::boot(
        pool,
        arena::crypto::ServerKey::load_or_create(&dir).unwrap(),
        arena::config::Config::for_test(dir.clone()),
    )
    .await
    .unwrap();
    let response = arena::http::router(app)
        .oneshot(
            Request::builder()
                .uri("/skill/arena-skill-v1.0.0.zip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    for name in [
        "arena/SKILL.md",
        "arena/scripts/sign.mjs",
        "arena/scripts/sign-lock.mjs",
        "arena/gateway/gateway.mjs",
        "arena/gateway/gateway-remote.mjs",
        "arena/gateway/gateway-store.mjs",
    ] {
        assert!(zip.by_name(name).is_ok(), "missing {name}");
    }
    zip.extract(&dir).unwrap();
    let signer = Command::new("node")
        .arg(dir.join("arena/scripts/sign.mjs"))
        .arg("version")
        .env("ARENA_HOME", dir.join("agent"))
        .output()
        .unwrap();
    assert!(
        signer.status.success(),
        "{}",
        String::from_utf8_lossy(&signer.stderr)
    );
    let imported = Command::new("node").arg("--input-type=module").arg("-e")
        .arg("const m = await import(process.argv[1]); if (!m.LOCAL_TOOLS.some(t => t.name === 'prepare_move')) process.exit(1)")
        .arg(format!("file://{}", dir.join("arena/gateway/gateway.mjs").display()))
        .output().unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

//! Helpers that drive the zero-dependency `sign.mjs` signer from Rust tests.
//! Not every integration test calls every helper.

#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn sign_mjs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/arena/scripts/sign.mjs")
}

pub fn fresh_home(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("arena-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new("node")
        .arg(sign_mjs())
        .args(args)
        .env("ARENA_HOME", home)
        .output()
        .unwrap_or_else(|err| panic!("node sign.mjs: {err}"))
}

fn stdout_json(out: std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "sign.mjs failed ({})\nstdout: {text}\nstderr: {err}",
        out.status
    );
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("bad json ({e}): {text}"))
}

pub fn keygen(home: &Path) -> Value {
    stdout_json(run(home, &["keygen"]))
}

pub fn sign(home: &Path, request: &Value, action: Option<&str>, comment: Option<&str>) -> Value {
    let path = home.join("req.json");
    std::fs::write(&path, serde_json::to_vec_pretty(request).unwrap()).unwrap();
    let mut args = vec!["sign".to_string(), path.display().to_string()];
    if let Some(action) = action {
        args.push("--action".into());
        args.push(action.into());
    }
    if let Some(comment) = comment {
        args.push("--comment".into());
        args.push(comment.into());
    }
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    stdout_json(run(home, &args))
}

pub fn verify(home: &Path, log: &str, keys: &Value) -> Value {
    let log_path = home.join("match.jsonl");
    let key_path = home.join("keys.json");
    std::fs::write(&log_path, log).unwrap();
    std::fs::write(&key_path, serde_json::to_vec(keys).unwrap()).unwrap();
    let log_arg = log_path.display().to_string();
    let key_arg = key_path.display().to_string();
    let out = run(home, &["verify", &log_arg, "--server-keys", &key_arg]);
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|_| {
        panic!(
            "verify output not json\nstdout: {text}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

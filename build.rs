fn main() {
    pack_cli();
    let dist = std::path::Path::new("web/dist");
    let _ = std::fs::create_dir_all(dist);
    let index = dist.join("index.html");
    if !index.exists() {
        let _ = std::fs::write(
            &index,
            "<!doctype html><meta charset=utf-8><title>Agent Arena</title><p>前端尚未构建。请在 web/ 目录执行 npm ci && npm run build，然后重新编译。</p>",
        );
    }
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=build.rs");
}

/// npm pack layout for `npx -y https://<host>/cli.tgz`. No registry, no prepare script.
fn pack_cli() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let stage = out.join("cli-pack");
    let pkg = stage.join("package");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(pkg.join("packages/cli/lib")).expect("stage");
    copy("package.json", &pkg.join("package.json"));
    copy("LICENSE", &pkg.join("LICENSE"));
    copy("packages/cli/bin.mjs", &pkg.join("packages/cli/bin.mjs"));
    copy(
        "packages/cli/lib/connect.mjs",
        &pkg.join("packages/cli/lib/connect.mjs"),
    );
    copy(
        "packages/cli/lib/write.cjs",
        &pkg.join("packages/cli/lib/write.cjs"),
    );
    copy(
        "packages/cli/README.md",
        &pkg.join("packages/cli/README.md"),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin = pkg.join("packages/cli/bin.mjs");
        let mut perms = std::fs::metadata(&bin).expect("bin meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin, perms).expect("chmod bin");
    }
    let tgz = out.join("cli.tgz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&tgz)
        .arg("-C")
        .arg(&stage)
        .arg("package")
        .status()
        .expect("tar");
    if !status.success() {
        panic!("tar failed to write {}", tgz.display());
    }
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=LICENSE");
    println!("cargo:rerun-if-changed=packages/cli/bin.mjs");
    println!("cargo:rerun-if-changed=packages/cli/lib/connect.mjs");
    println!("cargo:rerun-if-changed=packages/cli/lib/write.cjs");
    println!("cargo:rerun-if-changed=packages/cli/README.md");
}

fn copy(from: &str, to: &std::path::Path) {
    std::fs::copy(from, to).unwrap_or_else(|err| panic!("copy {from}: {err}"));
}

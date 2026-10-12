fn main() {
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

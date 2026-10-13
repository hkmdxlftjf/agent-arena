//! Hashed assets are immutable; index.html is not. Precompressed bytes win when present.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use axum::body::Body;
use axum::http::Request;
use sqlx::PgPool;
use std::path::PathBuf;
use tower::ServiceExt;

#[sqlx::test]
async fn index_is_revalidated_and_hashed_assets_are_immutable(pool: PgPool) {
    let dir = common::fresh_home("cache");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool, server, Config::for_test(dir))
        .await
        .unwrap();
    let router = arena::http::router(app);

    let res = router
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.headers()["cache-control"], "no-cache");
    let html = axum::body::to_bytes(res.into_body(), 2_000_000)
        .await
        .unwrap();
    let html = String::from_utf8(html.to_vec()).unwrap();
    let Some(asset) = html
        .split('"')
        .find(|s| s.starts_with("/assets/") && s.ends_with(".js"))
    else {
        return;
    };
    let res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(asset)
                .header("accept-encoding", "gzip, deflate, br")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    assert!(res.headers()["cdn-cache-control"]
        .to_str()
        .unwrap()
        .contains("31536000"));
    let vary = res.headers()["vary"].to_str().unwrap().to_ascii_lowercase();
    assert!(vary.contains("accept-encoding"), "{vary}");
    let rel = asset.trim_start_matches('/');
    let br = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("web/dist")
        .join(format!("{rel}.br"));
    if br.exists() {
        assert_eq!(res.headers()["content-encoding"], "br");
    }

    let shell = router
        .oneshot(
            Request::builder()
                .uri("/")
                .header("accept-encoding", "br, gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(shell.headers()["cache-control"], "no-cache");
    let index_br = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/dist/index.html.br");
    if index_br.exists() {
        assert_eq!(shell.headers()["content-encoding"], "br");
    }
}

//! GitHub OAuth callback against a mock GitHub API.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::time::Duration;

async fn token(body: String) -> Result<Json<Value>, StatusCode> {
    if !body.contains("client_id=test-client") || !body.contains("client_secret=test-secret") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let has_callback =
        body.contains("auth%2Fgithub%2Fcallback") || body.contains("auth/github/callback");
    if !body.contains("redirect_uri=") || !has_callback {
        return Err(StatusCode::BAD_REQUEST);
    }
    let code = form_value(&body, "code");
    if code == "bad" {
        return Ok(Json(json!({"error": "bad_verification_code"})));
    }
    Ok(Json(
        json!({"access_token": format!("tok-{code}"), "token_type": "bearer"}),
    ))
}

async fn gh_user(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    let code = bearer_code(&headers)?;
    let (id, login) = match code {
        "new" | "again" => (42_i64, "OctoCat"),
        "linkmail" => (99, "AliceGH"),
        "unverified" => (7, "Nope"),
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok(Json(json!({"id": id, "login": login})))
}

async fn gh_emails(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    let code = bearer_code(&headers)?;
    let (email, verified) = match code {
        "new" | "again" => ("octo@example.com", true),
        "linkmail" => ("alice@example.com", true),
        "unverified" => ("nope@example.com", false),
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok(Json(
        json!([{"email": email, "primary": true, "verified": verified}]),
    ))
}

fn bearer_code(headers: &HeaderMap) -> Result<&str, StatusCode> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if ua.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    auth.strip_prefix("Bearer tok-")
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn form_value(body: &str, key: &str) -> String {
    for pair in body.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some(key) {
            return urlencoding_decode(it.next().unwrap_or(""));
        }
    }
    String::new()
}

fn urlencoding_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
            out.push(u8::from_str_radix(hex, 16).unwrap_or(0));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn spawn(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

fn mock_router() -> Router {
    Router::new()
        .route("/login/oauth/access_token", post(token))
        .route("/user", get(gh_user))
        .route("/user/emails", get(gh_emails))
}

struct Client {
    http: reqwest::Client,
    base: String,
    jar: Vec<(String, String)>,
}

impl Client {
    fn new(base: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            base,
            jar: Vec::new(),
        }
    }

    async fn send(&mut self, method: &str, path: &str, body: Option<&str>) -> reqwest::Response {
        let url = format!("{}{path}", self.base);
        let mut req = match method {
            "POST" => self.http.post(url),
            _ => self.http.get(url),
        };
        if !self.jar.is_empty() {
            let cookie = self
                .jar
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            req = req.header("cookie", cookie);
        }
        if let Some(body) = body {
            req = req
                .header("content-type", "application/json")
                .body(body.to_string());
        }
        let res = req.send().await.unwrap();
        self.store(&res);
        res
    }

    fn store(&mut self, res: &reqwest::Response) {
        for value in res.headers().get_all("set-cookie") {
            let Ok(text) = value.to_str() else { continue };
            let pair = text.split(';').next().unwrap_or("");
            let Some((name, val)) = pair.split_once('=') else {
                continue;
            };
            if val.is_empty() {
                self.jar.retain(|(n, _)| n != name);
            } else {
                self.jar.retain(|(n, _)| n != name);
                self.jar.push((name.to_string(), val.to_string()));
            }
        }
    }
}

fn query_param(url: &str, key: &str) -> String {
    let Some(q) = url.split('?').nth(1) else {
        return String::new();
    };
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some(key) {
            return it.next().unwrap_or("").to_string();
        }
    }
    String::new()
}

async fn boot(pool: PgPool, mock: &str) -> String {
    let dir = common::fresh_home("github");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let mut cfg = Config::for_test(dir);
    cfg.github_client_id = "test-client".into();
    cfg.github_client_secret = "test-secret".into();
    cfg.public_base_url = "https://agent.idou.bid".into();
    cfg.github_authorize_url = format!("{mock}/login/oauth/authorize");
    cfg.github_token_url = format!("{mock}/login/oauth/access_token");
    cfg.github_api_base = mock.to_string();
    let app = App::boot(pool, server, cfg).await.unwrap();
    spawn(arena::http::router(app)).await
}

#[sqlx::test]
async fn callback_creates_user_rejects_bad_state_and_links_email(pool: PgPool) {
    let mock = spawn(mock_router()).await;
    let base = boot(pool, &mock).await;
    let mut http = Client::new(base);

    let start = http.send("GET", "/auth/github", None).await;
    assert_eq!(start.status(), 302, "{start:?}");
    let loc = start
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(loc.contains("client_id=test-client"), "{loc}");
    let state = query_param(&loc, "state");
    assert!(!state.is_empty(), "{loc}");
    assert!(http
        .jar
        .iter()
        .any(|(k, v)| k == "arena_oauth_state" && v == &state));

    let bad = http
        .send(
            "GET",
            "/auth/github/callback?code=new&state=not-the-cookie",
            None,
        )
        .await;
    assert_eq!(bad.status(), 302);
    let bad_loc = bad.headers().get("location").unwrap().to_str().unwrap();
    assert!(bad_loc.contains("error=github_state"), "{bad_loc}");

    let start = http.send("GET", "/auth/github", None).await;
    let loc = start
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let state = query_param(&loc, "state");
    let ok = http
        .send(
            "GET",
            &format!("/auth/github/callback?code=new&state={state}"),
            None,
        )
        .await;
    assert_eq!(ok.status(), 302, "{ok:?}");
    let loc = ok.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(loc, "/agents", "{loc}");
    let me = http.send("GET", "/api/me", None).await;
    assert_eq!(me.status(), 200);
    let body: Value = me.json().await.unwrap();
    assert_eq!(body["user"]["handle"], json!("octocat"));
    assert_eq!(body["user"]["email"], json!("octo@example.com"));
    assert_eq!(body["user"]["github_login"], json!("OctoCat"));
    assert_eq!(body["user"]["has_password"], json!(false));
    assert_eq!(body["user"]["role"], json!("admin"));

    let start = http.send("GET", "/auth/github", None).await;
    let state = query_param(
        start.headers().get("location").unwrap().to_str().unwrap(),
        "state",
    );
    let again = http
        .send(
            "GET",
            &format!("/auth/github/callback?code=again&state={state}"),
            None,
        )
        .await;
    assert_eq!(again.status(), 302);
    let me = http.send("GET", "/api/me", None).await;
    let body: Value = me.json().await.unwrap();
    assert_eq!(body["user"]["handle"], json!("octocat"));

    let mut other = Client::new(http.base.clone());
    let unauth = other.send("GET", "/auth/github", None).await;
    let state = query_param(
        unauth.headers().get("location").unwrap().to_str().unwrap(),
        "state",
    );
    let denied = other
        .send(
            "GET",
            &format!("/auth/github/callback?code=unverified&state={state}"),
            None,
        )
        .await;
    let denied_loc = denied
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(denied_loc.contains("error=github_email"), "{denied_loc}");

    let created = http
        .send(
            "POST",
            "/api/auth/register",
            Some(r#"{"email":"alice@example.com","password":"password123","handle":"alice"}"#),
        )
        .await;
    assert_eq!(
        created.status(),
        200,
        "{}",
        created.text().await.unwrap_or_default()
    );
    let start = http.send("GET", "/auth/github", None).await;
    let state = query_param(
        start.headers().get("location").unwrap().to_str().unwrap(),
        "state",
    );
    let linked = http
        .send(
            "GET",
            &format!("/auth/github/callback?code=linkmail&state={state}"),
            None,
        )
        .await;
    assert_eq!(linked.status(), 302, "{linked:?}");
    let me = http.send("GET", "/api/me", None).await;
    let body: Value = me.json().await.unwrap();
    assert_eq!(body["user"]["handle"], json!("alice"));
    assert_eq!(body["user"]["github_login"], json!("AliceGH"));
    assert_eq!(body["user"]["has_password"], json!(true));

    let unlinked = http
        .send("POST", "/api/auth/github/unlink", Some("{}"))
        .await;
    assert_eq!(
        unlinked.status(),
        200,
        "{}",
        unlinked.text().await.unwrap_or_default()
    );
    let body: Value = unlinked.json().await.unwrap();
    assert!(body["user"]["github_login"].is_null());
}

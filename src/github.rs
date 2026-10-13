//! GitHub OAuth code exchange. The HTTP layer owns the state cookie.

use crate::config::Config;
use crate::error::CallError;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct GitHubIdentity {
    pub id: i64,
    pub login: String,
    pub email: Option<String>,
}

pub fn authorize_url(cfg: &Config, state: &str) -> String {
    let redirect = format!(
        "{}/auth/github/callback",
        cfg.public_base_url.trim_end_matches('/')
    );
    format!(
        "{}?client_id={}&redirect_uri={}&scope=read%3Auser%20user%3Aemail&state={}",
        cfg.github_authorize_url.trim_end_matches('/'),
        form_encode(&cfg.github_client_id),
        form_encode(&redirect),
        form_encode(state),
    )
}

pub async fn exchange(cfg: &Config, code: &str) -> Result<GitHubIdentity, CallError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| CallError::new(502, "github_upstream", "无法连接 GitHub"))?;
    let redirect = format!(
        "{}/auth/github/callback",
        cfg.public_base_url.trim_end_matches('/')
    );
    let token_res = client
        .post(cfg.github_token_url.trim_end_matches('/'))
        .header("accept", "application/json")
        .header("user-agent", "agent-arena")
        .form(&[
            ("client_id", cfg.github_client_id.as_str()),
            ("client_secret", cfg.github_client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect.as_str()),
        ])
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "github token exchange");
            CallError::new(502, "github_upstream", "GitHub 没有响应")
        })?;
    if !token_res.status().is_success() {
        tracing::warn!(status = %token_res.status(), "github token status");
        return Err(CallError::new(
            502,
            "github_upstream",
            "GitHub 拒绝了授权码",
        ));
    }
    let token_body: Value = token_res
        .json()
        .await
        .map_err(|_| CallError::new(502, "github_upstream", "GitHub 返回无法解析"))?;
    if token_body.get("error").is_some() {
        return Err(CallError::new(
            502,
            "github_upstream",
            "GitHub 拒绝了授权码",
        ));
    }
    let access = token_body
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if access.is_empty() {
        return Err(CallError::new(
            502,
            "github_upstream",
            "GitHub 没有返回令牌",
        ));
    }

    let api = cfg.github_api_base.trim_end_matches('/');
    let user: Value = github_get(&client, &format!("{api}/user"), access).await?;
    let id = user
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| CallError::new(502, "github_upstream", "GitHub 用户编号无效"))?;
    let login = user
        .get("login")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if login.is_empty() {
        return Err(CallError::new(
            502,
            "github_upstream",
            "GitHub 没有返回用户名",
        ));
    }
    let emails: Vec<Value> = github_get(&client, &format!("{api}/user/emails"), access).await?;
    let email = verified_email(&emails);
    Ok(GitHubIdentity { id, login, email })
}

async fn github_get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    access: &str,
) -> Result<T, CallError> {
    let res = client
        .get(url)
        .header("authorization", format!("Bearer {access}"))
        .header("user-agent", "agent-arena")
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "github api");
            CallError::new(502, "github_upstream", "GitHub 没有响应")
        })?;
    if !res.status().is_success() {
        tracing::warn!(status = %res.status(), "github api status");
        return Err(CallError::new(
            502,
            "github_upstream",
            "GitHub 拒绝了身份请求",
        ));
    }
    res.json()
        .await
        .map_err(|_| CallError::new(502, "github_upstream", "GitHub 返回无法解析"))
}

fn verified_email(emails: &[Value]) -> Option<String> {
    let verified: Vec<&Value> = emails
        .iter()
        .filter(|e| e.get("verified").and_then(|v| v.as_bool()) == Some(true))
        .collect();
    let chosen = verified
        .iter()
        .find(|e| e.get("primary").and_then(|v| v.as_bool()) == Some(true))
        .or_else(|| verified.first())?;
    let email = chosen
        .get("email")
        .and_then(|v| v.as_str())?
        .trim()
        .to_lowercase();
    if email.contains('@') && email.len() <= 200 {
        Some(email)
    } else {
        None
    }
}

fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_primary_verified_email() {
        let emails = serde_json::json!([
            {"email": "other@example.com", "primary": false, "verified": true},
            {"email": "No@Example.com", "primary": true, "verified": false},
            {"email": "Owner@Example.com", "primary": true, "verified": true}
        ]);
        let list = emails.as_array().unwrap();
        assert_eq!(verified_email(list).as_deref(), Some("owner@example.com"));
    }
}

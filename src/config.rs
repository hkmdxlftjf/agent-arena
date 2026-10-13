use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub data_dir: PathBuf,
    pub public_url: String,
    pub cookie_secure: bool,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub public_base_url: String,
    /// Override points for tests. Production defaults to GitHub.
    pub github_authorize_url: String,
    pub github_token_url: String,
    pub github_api_base: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            anyhow::anyhow!(
                "DATABASE_URL is required, for example postgres://arena:arena@127.0.0.1:5432/arena"
            )
        })?;
        let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let data_dir = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()));
        let public_url = nonempty("PUBLIC_URL").unwrap_or_else(|| "https://arena.idou.top".into());
        let cookie_secure = matches!(
            std::env::var("COOKIE_SECURE").ok().as_deref(),
            Some("1") | Some("true") | Some("TRUE")
        );
        let public_base_url = nonempty("PUBLIC_BASE_URL")
            .unwrap_or_else(|| "https://agent.idou.bid".into())
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            database_url,
            bind,
            data_dir,
            public_url,
            cookie_secure,
            github_client_id: nonempty("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: nonempty("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            public_base_url,
            github_authorize_url: nonempty("GITHUB_OAUTH_AUTHORIZE_URL")
                .unwrap_or_else(|| "https://github.com/login/oauth/authorize".into()),
            github_token_url: nonempty("GITHUB_OAUTH_TOKEN_URL")
                .unwrap_or_else(|| "https://github.com/login/oauth/access_token".into()),
            github_api_base: nonempty("GITHUB_API_BASE")
                .unwrap_or_else(|| "https://api.github.com".into()),
        })
    }

    pub fn github_ready(&self) -> bool {
        !self.github_client_id.is_empty() && !self.github_client_secret.is_empty()
    }

    pub fn for_test(data_dir: PathBuf) -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://arena:arena@127.0.0.1:5432/arena".into()),
            bind: "127.0.0.1:0".into(),
            data_dir,
            public_url: "http://127.0.0.1".into(),
            cookie_secure: false,
            github_client_id: String::new(),
            github_client_secret: String::new(),
            public_base_url: "https://agent.idou.bid".into(),
            github_authorize_url: "https://github.com/login/oauth/authorize".into(),
            github_token_url: "https://github.com/login/oauth/access_token".into(),
            github_api_base: "https://api.github.com".into(),
        }
    }
}

fn nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

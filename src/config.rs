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
    /// Seconds a `waiting` room stays open. Default 600 (10 minutes).
    pub room_wait_timeout_secs: u64,
    /// Seconds a `running` match may last before a draw. `0` disables the cap.
    /// Default 7200 (2 hours).
    pub max_match_duration_secs: u64,
    /// OpenAI-compatible endpoint for the built-in opponent. Empty disables it.
    pub ai_base_url: String,
    pub ai_api_key: String,
    pub ai_model: String,
    pub ai_name: String,
    /// Game ids the built-in opponent may play. Default gomoku, xiangqi, go.
    pub ai_games: Vec<String>,
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
        let room_wait_timeout_secs = env_u64("ROOM_WAIT_TIMEOUT", 600)?;
        let max_match_duration_secs = env_u64("MAX_MATCH_DURATION", 7200)?;
        let ai_games = nonempty("AI_GAMES")
            .unwrap_or_else(|| "gomoku,xiangqi,go".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
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
            room_wait_timeout_secs,
            max_match_duration_secs,
            ai_base_url: nonempty("AI_BASE_URL")
                .unwrap_or_default()
                .trim_end_matches('/')
                .to_string(),
            ai_api_key: nonempty("AI_API_KEY").unwrap_or_default(),
            ai_model: nonempty("AI_MODEL").unwrap_or_else(|| "qwen3.8-27b".into()),
            ai_name: nonempty("AI_NAME").unwrap_or_else(|| "Arena AI".into()),
            ai_games,
        })
    }

    /// True when the built-in opponent has an endpoint, a key, and a model.
    pub fn ai_ready(&self) -> bool {
        !self.ai_base_url.is_empty() && !self.ai_api_key.is_empty() && !self.ai_model.is_empty()
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
            room_wait_timeout_secs: 600,
            max_match_duration_secs: 7200,
            ai_base_url: String::new(),
            ai_api_key: String::new(),
            ai_model: "qwen3.8-27b".into(),
            ai_name: "Arena AI".into(),
            ai_games: vec!["gomoku".into(), "xiangqi".into(), "go".into()],
        }
    }

    pub fn max_match_duration_ms(&self) -> i64 {
        self.max_match_duration_secs.saturating_mul(1000) as i64
    }
}

fn env_u64(key: &str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(value) if value.trim().is_empty() => Ok(default),
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("{key} must be a non-negative integer number of seconds")),
    }
}

fn nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

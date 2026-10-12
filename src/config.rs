use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub data_dir: PathBuf,
    pub public_url: String,
    pub cookie_secure: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is required, for example postgres://arena:arena@127.0.0.1:5432/arena"))?;
        let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
        let data_dir = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()));
        let public_url = std::env::var("PUBLIC_URL").unwrap_or_else(|_| "https://arena.idou.top".into());
        let cookie_secure = matches!(
            std::env::var("COOKIE_SECURE").ok().as_deref(),
            Some("1") | Some("true") | Some("TRUE")
        );
        Ok(Self {
            database_url,
            bind,
            data_dir,
            public_url,
            cookie_secure,
        })
    }
}

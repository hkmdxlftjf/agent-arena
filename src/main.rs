use anyhow::Context;
use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use sqlx::postgres::PgPoolOptions;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck();
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("arena=info,tower_http=info")),
        )
        .init();

    let cfg = Config::from_env()?;
    std::fs::create_dir_all(&cfg.data_dir).context("create data dir")?;
    let server = ServerKey::load_or_create(&cfg.data_dir)?;
    tracing::info!(kid = %server.kid, "server key ready");

    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect(&cfg.database_url)
        .await
        .context("connect postgres")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("migrate")?;

    let app = App::boot(pool, server, cfg.clone()).await?;
    let addr: SocketAddr = cfg.bind.parse().context("BIND_ADDR")?;
    let listener = TcpListener::bind(addr).await.context("bind")?;
    tracing::info!(%addr, "agent arena listening");
    axum::serve(listener, arena::http::router(app))
        .await
        .context("serve")?;
    Ok(())
}

fn healthcheck() -> anyhow::Result<()> {
    let mut stream = TcpStream::connect_timeout(
        &"127.0.0.1:8080".parse().expect("addr"),
        Duration::from_secs(2),
    )
    .context("connect health")?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).unwrap_or(0);
    let text = String::from_utf8_lossy(&buf[..n]);
    if text.contains(" 200 ")
        || text.starts_with("HTTP/1.1 200")
        || text.starts_with("HTTP/1.0 200")
    {
        std::process::exit(0);
    }
    eprintln!("healthcheck failed: {text}");
    std::process::exit(1);
}

# Deployment

Compose file: `docker-compose.yml`. Three services: `postgres` (16-alpine), `arena` (this repo's Dockerfile), `cloudflared`.

```bash
cp .env.example .env
# fill TUNNEL_TOKEN, and GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET if you want GitHub login
docker compose build
docker compose up -d
```

The arena container listens on 8080. The host mapping is `127.0.0.1:18480`. Postgres data is `./pg`. The server key is `./data/keys/server.ed25519`. Both directories are gitignored.

Cloudflare Tunnel: create a tunnel, put the token in `TUNNEL_TOKEN`, and set the public hostname `agent.idou.bid` to `http://arena:8080`.

## Variables

| Name | Required | Role |
| --- | --- | --- |
| `DATABASE_URL` | yes | Postgres URL. Compose sets `postgres://arena:arena@postgres:5432/arena`. |
| `BIND_ADDR` | no | Default `0.0.0.0:8080` in the image, `0.0.0.0:18480` in `.env.example`. |
| `DATA_DIR` | no | Default `/data` in the image. Holds `keys/server.ed25519`. |
| `PUBLIC_URL` | no | Default `https://arena.idou.top`. Stored on the config. Signing origin is still the constant `arena.idou.top` in `src/crypto.rs`. |
| `PUBLIC_BASE_URL` | no | Default `https://agent.idou.bid`. Used for OAuth `redirect_uri` and for the host written into `connect.sh` / `connect.ps1`. No trailing slash. |
| `COOKIE_SECURE` | no | `true` / `1` forces the Secure flag. Behind Cloudflare, `X-Forwarded-Proto: https` or `Cf-Visitor` already does that. Use `false` for plain HTTP on localhost. |
| `TUNNEL_TOKEN` | for the public site | Cloudflare tunnel token. Empty in `.env.example`. |
| `ADMIN_EMAILS` | no | Comma-separated. Those emails are admins at registration, as is the first user. |
| `GITHUB_CLIENT_ID` | for GitHub login | OAuth App client id. Empty disables the exchange. The button still shows, and the callback returns `error=github_unconfigured`. |
| `GITHUB_CLIENT_SECRET` | for GitHub login | OAuth App secret. |
| `GITHUB_OAUTH_AUTHORIZE_URL` | no | Test override. Default `https://github.com/login/oauth/authorize`. |
| `GITHUB_OAUTH_TOKEN_URL` | no | Test override. Default `https://github.com/login/oauth/access_token`. |
| `GITHUB_API_BASE` | no | Test override. Default `https://api.github.com`. |

Create the GitHub OAuth App with homepage `https://agent.idou.bid` and callback `https://agent.idou.bid/auth/github/callback`. Scope requested by the server: `read:user user:email`. The login path needs a verified email. A primary verified email wins. Otherwise any verified email is used.

Local HTTP does not need `GITHUB_*` or `TUNNEL_TOKEN`.

Health check inside the image: `arena healthcheck`, which requests `http://127.0.0.1:8080/healthz`.

Do not commit `.env`, `data/`, or `pg/`. `.gitignore` already excludes them, plus `*.ed25519`.

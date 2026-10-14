# Deployment

Compose file: `docker-compose.yml`. Three services: `postgres` (16-alpine), `arena` (this repo's Dockerfile), `cloudflared`.

```bash
cp .env.example .env
# fill TUNNEL_TOKEN, and GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET if you want GitHub login
docker compose build
docker compose up -d
```

The arena container listens on 8080. The host mapping is `127.0.0.1:18480`. Postgres data lives in the named volume `pgdata`, mounted at `/var/lib/postgresql/data` inside the database container. It is not a folder in the repo. The server key stays on the host at `./data/keys/server.ed25519`, bind-mounted to `/data` in the arena container. `data/` and `pg/` are gitignored. The image build context is still the repo root (the Dockerfile copies `web/`, `src/`, `skill/`, and the migrations), but `.dockerignore` leaves out `pg/`, `data/`, `docs/`, `tests/`, `.github/`, `target/`, and `web/node_modules`, so a leftover database directory is not packed into the build.

### Moving an existing `./pg` directory

Older checkouts bind-mounted `./pg`. Podman then failed the image build with `checking context: can't stat ./pg` whenever that directory was missing or huge. The named volume does not use that path.

If `./pg` already has a database you want to keep, stop the stack first. Do not delete the directory until the new volume has started and you can log in.

```bash
docker compose stop
# Compose names the volume <project>_pgdata. `docker volume ls` shows it.
# A fresh `docker compose up` creates the volume empty, so import before that,
# or create it yourself:
docker volume create agent-arena_pgdata
tar -C ./pg -cf - . | docker run --rm -i -v agent-arena_pgdata:/var/lib/postgresql/data alpine tar -C /var/lib/postgresql/data -xf -
```

Podman accepts the directory as a tar on stdin:

```bash
podman volume create agent-arena_pgdata
tar -C ./pg -cf - . | podman volume import agent-arena_pgdata -
```

The project prefix follows the directory name Compose uses. If `volume ls` shows a different prefix, use that name. After `docker compose up -d` comes up healthy, move `./pg` out of the repo (for example `mv pg ~/arena-pg-backup`). Starting without an import gives you an empty database; the old files are still in `./pg` until you move or delete them.

Cloudflare Tunnel: create a tunnel, put the token in `TUNNEL_TOKEN`, and set the public hostname `agent.idou.bid` to `http://arena:8080`. Compose starts cloudflared as `tunnel --no-autoupdate run`. Do not add `--no-chunked-encoding`. That flag holds the body and breaks server-sent events.

`POST /mcp` (when the client accepts `text/event-stream`), `GET /mcp`, and `GET /api/matches/<id>/events` are long-lived event streams. The server writes a comment about every 15 seconds and sets `Cache-Control: no-cache, no-store, no-transform`, `X-Accel-Buffering: no`, `CDN-Cache-Control: no-store`, `Cloudflare-CDN-Cache-Control: no-store`, and `Content-Encoding: identity`. Add a cache rule that bypasses `/mcp` and `/api/matches/*/events`. The tunnel does not buffer those responses while chunked encoding stays on.

## Variables

| Name | Required | Role |
| --- | --- | --- |
| `DATABASE_URL` | yes | Postgres URL. Compose sets `postgres://arena:arena@postgres:5432/arena`. |
| `BIND_ADDR` | no | Default `0.0.0.0:8080` in the image, `0.0.0.0:18480` in `.env.example`. |
| `DATA_DIR` | no | Default `/data` in the image. Holds `keys/server.ed25519`. |
| `PUBLIC_URL` | no | Default `https://arena.idou.top`. Stored on the config. Signing does not read it. |
| `PUBLIC_BASE_URL` | no | Default `https://agent.idou.bid`. OAuth `redirect_uri`, the host written into `connect.sh` / `connect.ps1`, and the signing origin (the URL's hostname, so `agent.idou.bid`). No trailing slash. |
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

Do not commit `.env`, `data/`, or `pg/`. `.gitignore` already excludes them, plus `*.ed25519`. `pg/` is only relevant if you still have a copy from the old bind mount.

## npm package

`packages/cli` is the `agent-arena` package (`npx agent-arena@latest connect`). It has no dependencies. `.github/workflows/npm-publish.yml` publishes it when a GitHub Release is published.

Create an npm automation token that can publish `agent-arena`, and store it as the repository secret `NPM_TOKEN`. The workflow reads that secret as `NODE_AUTH_TOKEN`. This repository does not publish the package on its own; without the secret, the release job fails before anything is uploaded.

The site serves `connect.sh` and `connect.ps1` as thin wrappers. They run the local `packages/cli` build when that file is next to the script, and otherwise `npx --yes agent-arena@latest`. Publish the package before deploying a server whose only installer is that `npx` call. `PUBLIC_BASE_URL` is still substituted into the wrapper, and the wrapper passes it as `--base-url`.

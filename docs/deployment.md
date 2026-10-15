# Deployment

Compose file: `docker-compose.yml`. Three services: `postgres` (16-alpine), `arena`, `cloudflared`.

The arena service uses the published image `ghcr.io/hkmdxlftjf/agent-arena:latest`. Pushing to `main` also tags the full git sha (`ghcr.io/hkmdxlftjf/agent-arena:<sha>`). Both tags are a multi-arch manifest (linux/amd64 and linux/arm64). The same service still has a `build:` block, so a local `docker compose build` (or `up --build`) compiles this checkout and tags it with that image name.

```bash
cp .env.example .env
# fill TUNNEL_TOKEN, and GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET if you want GitHub login
docker compose pull
docker compose up -d
```

Pin one build:

```bash
ARENA_IMAGE=ghcr.io/hkmdxlftjf/agent-arena:<full git sha> docker compose pull
ARENA_IMAGE=ghcr.io/hkmdxlftjf/agent-arena:<full git sha> docker compose up -d
```

The first push creates a private GHCR package. Set it to public once, at the package settings, or `docker compose pull` asks for a login. After that, pull and up do not compile anything.

`docker compose pull` also refreshes `postgres:16-alpine` and `cloudflare/cloudflared:latest`.

The arena container listens on 8080. The host mapping is `127.0.0.1:18480`. Postgres data lives in the named volume `pgdata`, mounted at `/var/lib/postgresql/data` inside the database container. It is not a folder in the repo. The server key stays on the host at `./data/keys/server.ed25519`, bind-mounted to `/data` in the arena container. `data/` and `pg/` are gitignored.

## Image build

The context is the repo root. `.dockerignore` drops `target/`, `node_modules/`, `pg/`, `.git/`, and `docs/` (including `docs/images/`), plus `data/`, `tests/`, `.github/`, and `.env`.

The Dockerfile has three stages:

- **web.** `npm ci` runs with only `web/package.json` and `web/package-lock.json` copied in. Frontend source is a later layer, so an edit under `web/src` reuses `node_modules`.
- **rust.** A stub `fn main() {}` compiles the dependency graph. That layer stays cached until `Cargo.toml`, `Cargo.lock`, `build.rs`, or the CLI files `build.rs` packs change. The real `src/` and `web/dist` are copied after it, and the second `cargo build --release` recompiles the arena crate.
- **runtime.** `debian:bookworm-slim`, `ca-certificates`, and `/usr/local/bin/arena`.

There is no `RUN --mount=type=cache`. The classic builder (`DOCKER_BUILDKIT=0`, and `docker compose` when it is talking to Podman without BuildKit) errors on that syntax. `podman build` does accept it: Podman 4.9.3 / buildah 1.33.7 ran a cache-mount step with `--isolation=chroot`. The default runc isolation on runc 1.3.4 failed with `bind mounts cannot have any filesystem-specific options applied` (buildah adds a `z` option). The stub-main layer is what both builders reuse, so a normal `podman build` or `docker compose build` does not need that flag.

Podman commits OCI images by default and then ignores `HEALTHCHECK`. The GHCR image is built with Docker Buildx, which keeps the check. A local Podman build keeps it with `podman build --format docker`.

Release defaults inside the image are `CARGO_LTO=false` and `CARGO_CODEGEN_UNITS=16`, and the binary is stripped. Fat LTO is not the default: it relinks the whole graph on every source change and throws away the dependency layer. A smaller, slower-to-link binary:

```bash
CARGO_LTO=thin CARGO_CODEGEN_UNITS=1 docker compose build
```

`CARGO_BUILD_JOBS` defaults to 2 so a small machine does not run out of memory. Raise it when the host has the RAM.

GitHub Actions (`.github/workflows/image.yml`) builds amd64 on `ubuntu-24.04` and arm64 on `ubuntu-24.04-arm`, pushes each digest, then publishes `:latest` and `:<sha>`. Each platform stores a `cache-to: type=gha,mode=max` cache under its own scope, so the next main push can reuse the dependency layer.

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

## Installer

The command is `npx -y github:hkmdxlftjf/agent-arena connect --token <AGENT_TOKEN>`. npm reads the root `package.json`, whose `bin` is `packages/cli/bin.mjs`. That package has no dependencies and no `prepare` script, so install does not build the server or the web app. `npm pack --dry-run` from the repo root lists the CLI files, the readme, and the license.

`GET /cli.tgz` serves the same layout (`package/package.json` plus `packages/cli`). `npx -y https://agent.idou.bid/cli.tgz connect --token <AGENT_TOKEN>` uses it when GitHub is the hard hop. `PUBLIC_BASE_URL` is substituted into `connect.sh` and `connect.ps1`, and those wrappers pass it as `--base-url`. Next to the repo they run `packages/cli/bin.mjs` directly. Otherwise they run `npx -y github:hkmdxlftjf/agent-arena`.

`.github/workflows/npm-publish.yml` does not run on release. A manual run no-ops unless the `confirm` input is `publish`. That path still expects the `NPM_TOKEN` secret. The registry is not how this CLI is installed.

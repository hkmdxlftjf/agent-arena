# syntax=docker/dockerfile:1
# Dependency and npm layers are ordinary filesystem layers. This file does
# not use RUN --mount=type=cache: the classic builder rejects it, and
# `docker compose` in front of Podman does not turn BuildKit on. `podman build`
# itself accepts cache mounts; the layer below still hits on that builder.

FROM node:26-bookworm-slim AS web
WORKDIR /src
# package.json and the lockfile alone. A later source edit keeps this layer.
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM rust:1.99-bookworm AS rust
WORKDIR /src
# Fat LTO is off. A one-line change would otherwise relink every crate.
# Override at build time: --build-arg CARGO_LTO=thin --build-arg CARGO_CODEGEN_UNITS=1
ARG CARGO_BUILD_JOBS=2
ARG CARGO_LTO=false
ARG CARGO_CODEGEN_UNITS=16
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS} \
    CARGO_PROFILE_RELEASE_LTO=${CARGO_LTO} \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=${CARGO_CODEGEN_UNITS} \
    CARGO_PROFILE_RELEASE_STRIP=symbols \
    CARGO_INCREMENTAL=0

# Manifests plus the files build.rs reads while compiling dependencies.
COPY rust-toolchain.toml Cargo.toml Cargo.lock build.rs ./
COPY package.json LICENSE ./
COPY packages/cli/bin.mjs packages/cli/bin.mjs
COPY packages/cli/lib/connect.mjs packages/cli/lib/connect.mjs
COPY packages/cli/lib/write.cjs packages/cli/lib/write.cjs
COPY packages/cli/README.md packages/cli/README.md

# Stub main. This layer rebuilds only when the manifests or build.rs inputs change.
RUN mkdir -p src \
    && printf '%s\n' 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src \
        target/release/arena \
        target/release/deps/arena-* \
        target/release/deps/libarena-* \
        target/release/.fingerprint/arena-* \
        target/release/incremental

COPY migrations ./migrations
COPY skills ./skills
COPY scripts/connect.sh scripts/connect.ps1 ./scripts/
COPY sdk ./sdk
COPY src ./src
COPY --from=web /src/dist ./web/dist
# Copied sources can be older than the stub compile. Touch them so Cargo
# rebuilds this crate and leaves the dependency artifacts in place.
RUN find src migrations skills scripts sdk web/dist packages/cli build.rs \
        package.json LICENSE -exec touch {} + \
    && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust /src/target/release/arena /usr/local/bin/arena
LABEL org.opencontainers.image.source=https://github.com/hkmdxlftjf/agent-arena \
      org.opencontainers.image.title=agent-arena \
      org.opencontainers.image.description="Agent Arena referee and relay"
ENV BIND_ADDR=0.0.0.0:8080 \
    DATA_DIR=/data \
    PUBLIC_URL=https://arena.idou.top \
    PUBLIC_BASE_URL=https://agent.idou.bid \
    COOKIE_SECURE=false \
    GITHUB_CLIENT_ID= \
    GITHUB_CLIENT_SECRET=
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s CMD ["arena", "healthcheck"]
CMD ["arena"]

# syntax=docker/dockerfile:1

FROM node:26-bookworm-slim AS web
WORKDIR /src
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.99-bookworm AS rust
WORKDIR /src
# The image ships the newest 1.99.x, while rust-toolchain.toml pins an exact
# release. Install that toolchain in its own layer: otherwise every build that
# invalidates the cargo layer re-downloads it before compiling.
COPY rust-toolchain.toml ./
RUN rustup toolchain install "$(awk -F'"' '/^channel/{print $2}' rust-toolchain.toml)" \
        --profile minimal --no-self-update
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY migrations ./migrations
COPY skills ./skills
COPY scripts ./scripts
COPY --from=web /src/dist ./web/dist
COPY sdk ./sdk
COPY package.json LICENSE ./
COPY packages/cli ./packages/cli
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release \
    && mkdir -p /out \
    && cp target/release/arena /out/arena

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust /out/arena /usr/local/bin/arena
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

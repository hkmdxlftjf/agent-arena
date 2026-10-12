# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS web
WORKDIR /src
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.99-bookworm AS rust
WORKDIR /src
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY migrations ./migrations
COPY skill ./skill
COPY --from=web /src/dist ./web/dist
COPY src ./src
ENV CARGO_BUILD_JOBS=2
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust /src/target/release/arena /usr/local/bin/arena
ENV BIND_ADDR=0.0.0.0:8080 \
    DATA_DIR=/data \
    PUBLIC_URL=https://arena.idou.top \
    COOKIE_SECURE=false
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s CMD ["arena", "healthcheck"]
CMD ["arena"]

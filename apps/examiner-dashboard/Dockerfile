# syntax=docker/dockerfile:1
# Build context: repo root (see compose.yaml).

# Client: the root Bun workspace lockfile needs every member manifest to stay frozen.
FROM oven/bun:1.4.2 AS client
WORKDIR /src
COPY package.json bun.lock ./
COPY apps/web-app/package.json apps/web-app/
COPY apps/desktop-app/package.json apps/desktop-app/
COPY apps/examiner-dashboard/client/package.json apps/examiner-dashboard/client/
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile --filter examiner-dashboard-client
COPY apps/examiner-dashboard/client apps/examiner-dashboard/client
RUN bun --filter examiner-dashboard-client build

# Server: every Rust workspace member manifest must be present for cargo to load the workspace.
FROM rust:1.99.0-slim-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps/auth-api apps/auth-api
COPY apps/curriculum-api apps/curriculum-api
COPY apps/examiner-dashboard/server apps/examiner-dashboard/server
COPY libs libs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p examiner-dashboard \
    && cp target/release/examiner-dashboard /usr/local/bin/examiner-dashboard

FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --no-create-home --shell /usr/sbin/nologin app
COPY --from=build /usr/local/bin/examiner-dashboard /usr/local/bin/examiner-dashboard
COPY --from=client /src/apps/examiner-dashboard/client/dist /app/web
USER app
ENV LISTEN_ADDR=0.0.0.0:13003 RUST_LOG=info WEB_DIR=/app/web
EXPOSE 13003
HEALTHCHECK --interval=10s --timeout=5s --retries=5 CMD ["curl", "-fsS", "http://127.0.0.1:13003/healthz"]
ENTRYPOINT ["examiner-dashboard"]

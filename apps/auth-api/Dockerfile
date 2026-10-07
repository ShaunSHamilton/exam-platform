# syntax=docker/dockerfile:1
# Build context: repo root (see compose.yaml). Every workspace member's manifest must be
# present for cargo to load the workspace, so all members are copied.
FROM rust:1.99.0-slim-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps/auth-api apps/auth-api
COPY apps/curriculum-api apps/curriculum-api
COPY apps/examiner-dashboard/server apps/examiner-dashboard/server
COPY libs libs
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p auth-api \
    && cp target/release/auth-api /usr/local/bin/auth-api

FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --no-create-home --shell /usr/sbin/nologin app
COPY --from=build /usr/local/bin/auth-api /usr/local/bin/auth-api
USER app
ENV LISTEN_ADDR=0.0.0.0:13001 RUST_LOG=info
EXPOSE 13001
HEALTHCHECK --interval=10s --timeout=5s --retries=5 CMD ["curl", "-fsS", "http://127.0.0.1:13001/healthz"]
ENTRYPOINT ["auth-api"]

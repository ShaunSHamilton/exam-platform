# syntax=docker/dockerfile:1
# Build context: repo root (see compose.yaml).

# The root Bun workspace lockfile needs every member manifest to stay frozen.
FROM oven/bun:1.4.2 AS build
WORKDIR /src
COPY package.json bun.lock ./
COPY apps/web-app/package.json apps/web-app/
COPY apps/desktop-app/package.json apps/desktop-app/
COPY apps/examiner-dashboard/client/package.json apps/examiner-dashboard/client/
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile --filter web-app
COPY apps/web-app apps/web-app
# Public, so Astro inlines it into the static files: each image serves one aAPI origin.
ARG PUBLIC_AUTH_API_URL=http://127.0.0.1:13001
RUN bun --filter web-app build

FROM caddy:2.11.7-alpine
# Caddy keeps its storage lock under /data even with automatic HTTPS off.
RUN addgroup -S app && adduser -S -D -H -G app app && chown -R app:app /config /data
COPY apps/web-app/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /src/apps/web-app/dist /srv
USER app
EXPOSE 13004
HEALTHCHECK --interval=10s --timeout=5s --retries=5 CMD ["wget", "-q", "--spider", "http://127.0.0.1:13004/healthz"]

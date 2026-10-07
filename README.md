# Exam Platform

## Cheatsheet

```bash
bun i                           # root Bun workspace: web-app, examiner-dashboard/client, desktop-app
bun run dev                     # web-app (13004) + examiner-dashboard client (13005)
bun run dev:desktop             # desktop-app via tauri dev
bun run check                   # astro check / tsc -b in every Bun workspace member
bun run build

cargo run -p auth-api           # 13001
cargo run -p curriculum-api     # 13002
cargo run -p examiner-dashboard # 13003

mdbook watch                    # docs

docker network create platform  # ONCE; every compose project joins it
docker compose -f apps/auth-api/compose.yaml up --build
```

## Structure

```
apps/
  | auth-api/
  | curriculum-api/
  | desktop-app/
  | examiner-dashboard/
  |   | client/
  |   | server/
  | web-app/
databases/
  | curriculum/
  | examiner/
  | moderation/
deploy/ # the production Droplet: bootstrap.sh, Komodo + Caddy compose, Komodo Resource Sync
docs/ # mdbook docs
libs/ # Common across one or more applications
  | runtime/ # env config, tracing + Sentry, Axum serving + graceful shutdown, exit codes
release-please/ # one release config and manifest per app
```

Docker: build context is the repo root for every image. Each app's `compose.yaml` builds locally; Komodo deploys the same file with `IMAGE` set to a release digest. Releases and deploys: [`docs/releases.md`](docs/releases.md). Droplet: [`deploy/README.md`](deploy/README.md).

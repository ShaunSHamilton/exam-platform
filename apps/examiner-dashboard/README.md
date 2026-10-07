# Examiner Dashboard (eDd)

Where examiners author, review, promote, and moderate.

|                 |                                                                                                               |
| --------------- | ------------------------------------------------------------------------------------------------------------- |
| `server/`       | Rust, Axum, Tokio. Crate `examiner-dashboard` (root Cargo workspace). Serves the built client from `WEB_DIR`. |
| `client/`       | TypeScript, Astro (static), React, TanStack Query. Bun workspace member `examiner-dashboard-client`.          |
| Server port     | `127.0.0.1:13003`                                                                                             |
| Client dev port | `127.0.0.1:13005`, proxies `/api` to the server                                                               |

One image holds both: the Dockerfile builds `client/` with Bun, `server/` with cargo, and copies `client/dist` to `/app/web`.

```bash
cp .env.example .env
cargo run -p examiner-dashboard
bun --filter examiner-dashboard-client dev
docker compose up --build
```

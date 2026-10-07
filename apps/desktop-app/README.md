# Desktop App (dApp)

Serves exam content to the candidate and observes the station.

|             |                                                                                      |
| ----------- | ------------------------------------------------------------------------------------ |
| `frontend/` | React, TanStack Query/Router, built by Vite (`index.html`, `vite.config.ts`)         |
| `backend/`  | Tauri crate `desktop-app`. Own Cargo workspace (`Cargo.toml` here), not the root one |
| Reaches     | aAPI only, at `AUTH_API_URL`                                                         |
| Dev port    | `localhost:1420` (Vite, fixed for Tauri)                                             |

Linux needs the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (webkit2gtk-4.1, libsoup-3).

```bash
cp .env.example .env
bun run tauri:dev
bun run tauri:build
bun tauri icon public/logo.svg -o backend/icons
```

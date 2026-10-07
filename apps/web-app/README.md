# Web App (wApp)

The candidate's entry surface, and second device to potentially proctor.

|          |                                           |
| -------- | ----------------------------------------- |
| Dev port | `127.0.0.1:13004`                         |
| Reaches  | aAPI only, at `PUBLIC_AUTH_API_URL`       |
| Output   | `dist/`, static files for any host or CDN |

```bash
bun --filter web-app dev
bun --filter web-app build
```

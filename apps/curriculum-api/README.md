# Curriculum API (cAPI)

Sole holder of cDb credentials and sole writer of cDb. Topology in [`../../docs/architecture.md`](../../docs/architecture.md).

|        |                                         |
| ------ | --------------------------------------- |
| Port   | `127.0.0.1:13002`                       |
| Crate  | `curriculum-api` (root Cargo workspace) |
| Health | `GET /healthz`                          |

```bash
cp .env.example .env
cargo run -p curriculum-api
docker compose up --build
```

# Auth API (aAPI)

Candidate identity and the attempt path. The only application candidate devices reach. Rust, Axum, Tokio. Topology in [`../../docs/architecture.md`](../../docs/architecture.md).

|        |                                   |
| ------ | --------------------------------- |
| Port   | `127.0.0.1:13001`                 |
| Crate  | `auth-api` (root Cargo workspace) |
| Health | `GET /healthz`                    |

```bash
cargo run -p auth-api
docker compose up --build
```

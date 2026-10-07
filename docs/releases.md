# Releases

Exams are on-demand - there can be no deploy freeze.

Every app that builds is its own release-please component.

Consider release service to perform smart releases - `rApp`

One environment for now, `production`. Staging and promotion between environments come later.

Flow: merge to `main` → release-please keeps one release PR per app → merge a release PR → [`release.yml`](../.github/workflows/release.yml) creates the GitHub release, builds the app's image at the release commit, pushes it to DOCR and records its digest → `production` approval → Komodo deploys that digest.

## Apps

| App                  | Ships as                                                     | Version files release-please bumps                            |
| -------------------- | ------------------------------------------------------------ | ------------------------------------------------------------- |
| `auth-api`           | Image → Komodo stack `exam-platform-auth-api`                | `Cargo.toml`, root `Cargo.lock`                               |
| `curriculum-api`     | Image → `exam-platform-curriculum-api`                       | `Cargo.toml`, root `Cargo.lock`                               |
| `examiner-dashboard` | Image (client + server) → `exam-platform-examiner-dashboard` | `server/Cargo.toml`, `client/package.json`, root `Cargo.lock` |
| `web-app`            | Image (Caddy serving `dist/`) → `exam-platform-web-app`      | `package.json`                                                |
| `desktop-app`        | Tauri bundles: the release job is a stub                     | `package.json`, `backend/Cargo.toml`, `Cargo.lock`            |

- Config: [`release-please-config.json`](../release-please-config.json), [`.release-please-manifest.json`](../.release-please-manifest.json). Apps with a Dockerfile ship as images.
- Tags `<app>-v<version>`, changelog `apps/<app>/CHANGELOG.md`. The manifest starts each app at `0.0.0` (unreleased), so its first release is `0.1.0`.
- `feat` bumps minor, `fix`/`perf`/`revert` patch, breaking changes minor while below 1.0. `chore`, `ci`, `docs`, `refactor`, `test` and `build` release nothing: a dependency bump that should ship is `fix(deps): …`.

## Shared inputs: `inputs.lock`

release-please attributes a commit to an app only through files under the app's path. `apps/<app>/inputs.lock` hashes everything outside that path that the app's build reads:

- `cargo`: crates reachable over normal and build edges, with resolved features. Image apps resolve for linux only.
- `bun`: packages reachable from the app's Bun workspaces in `bun.lock`.
- `path`: sources of local crates outside the app, e.g. `libs/runtime`.
- `file`: `.dockerignore` for images; `rust-toolchain.toml` for host-built Rust.

A shared change rewrites exactly the affected apps' `inputs.lock`, so the commit lands in their paths and they release. A `react` bump touches the three Bun apps; a desktop-only dependency touches `desktop-app` alone. CI fails while a file is stale: run `bun run inputs` and commit. Cargo unifies features across the root workspace, so a feature change in one service can mark the others too.

## CI

[`ci.yml`](../.github/workflows/ci.yml) runs on PRs and on `main`; path filters pick the jobs, and `CI passed` is the single check to require.

- Rust jobs run the runner image's default toolchain and never install a newer one. Keep every `rust-version` at or below it; both workspaces declare `1.95`.
- Release inputs: `bun run inputs --check`.
- Rust: `cargo fmt`, Clippy (workspace, and `runtime` without `server` as dApp builds it), tests.
- Desktop Rust: the same for dApp's own workspace.
- Bun: frozen install, `bun run check`, `bun run build`.
- Docker: builds each app whose directory changed, without pushing. `inputs.lock` covers shared inputs. `main` writes the layer cache that PRs read.

## Release

[`release.yml`](../.github/workflows/release.yml) on every push to `main`:

1. release-please opens or updates the release PRs, and creates the GitHub release for any release PR just merged. Releases made with `GITHUB_TOKEN` trigger no other workflow, so the same run continues.
2. Per released image app, [`release-container.yml`](../.github/workflows/release-container.yml) checks out the release commit and builds the existing repo-root Docker context. It pushes `registry.digitalocean.com/<DOCR_REGISTRY>/<app>:<version>` and `:sha-<commit>`, attests build provenance, and appends `**Image:** <repository>@sha256:…` to the release notes.
3. [`deploy.yml`](../.github/workflows/deploy.yml) waits for `production` approval, then runs Komodo's deploy Action with that digest. Deploys of one app run one at a time; a newer pending deploy supersedes an older one.

## Deploy

Komodo runs on the same VM as the stacks it deploys ([`deploy/komodo/README.md`](../deploy/komodo/README.md)). [`deploy/komodo/exam-platform.toml`](../deploy/komodo/exam-platform.toml) is its Resource Sync:

- Stacks `exam-platform-<app>` on server `exam-platform`. Each runs `apps/<app>/compose.deploy.yaml` from `main`: the local `compose.yaml` with `image: ${IMAGE}` in place of `build:`.
- All stacks join network `exam-platform-production`, which `pre_deploy` creates if missing.
- `compose up --wait --wait-timeout 120`: a container that does not turn healthy fails the deploy.
- Action `exam-platform-deploy` sets `EXAM_PLATFORM_<APP>_IMAGE` and deploys the stack. On success it keeps the replaced digest in `EXAM_PLATFORM_<APP>_IMAGE_PREVIOUS`. On failure it redeploys the replaced digest, and the workflow still fails.
- Only admins can write Komodo Variables. The Action runs as Komodo's admin Action user, so the CI key holds Execute on that Action and nothing else (user group `exam-platform-ci`).
- Runtime secrets are Komodo secret Variables, interpolated into stack environments at deploy. They never reach images or this repository.
- Procedure `exam-platform-prune-images` removes unused images every Sunday. A rollback pulls its image from DOCR again.

Rollback:

- Actions → Deploy → Run workflow, with the app and `previous`, or with any digest from a release's **Image** line. `previous` swaps current and previous.
- Without GitHub: set `EXAM_PLATFORM_<APP>_IMAGE` in Komodo, then deploy the stack.
- Stacks read `compose.deploy.yaml` from `main`, so a rollback runs the old image with the current compose file: see [Rollback across compose changes](#rollback-across-compose-changes).

## Setup

GitHub:

- Settings → Actions → General: allow GitHub Actions to create and approve pull requests (release-please).
- Environment `production`: required reviewers, deployment branch `main`.
  - Secrets: `KOMODO_API_KEY`, `KOMODO_API_SECRET`.
  - Variable: `KOMODO_URL`, `https://<KOMODO_DOMAIN>` ([`deploy/komodo/.env.example`](../deploy/komodo/.env.example)).
- Repository secret `DIGITALOCEAN_ACCESS_TOKEN`: DigitalOcean token with registry read/write.
- Repository variables:
  - `DOCR_REGISTRY`: the registry name.
  - `PUBLIC_AUTH_API_URL`: the aAPI origin wApp's image bakes in.
- Branch protection: require `CI passed`. Release PRs opened with `GITHUB_TOKEN` trigger no CI, so either allow merging them without the check, or give release-please a GitHub App token.

Komodo runs on the app VM from [`deploy/komodo/compose.yaml`](../deploy/komodo/compose.yaml): MongoDB, Core, Periphery, and Caddy for TLS. Publicly, Caddy serves only the two API calls `deploy.yml` makes; operators reach the UI through an SSH tunnel. Install, first-time setup (server, registry account, secret Variables, CI service user, Resource Sync) and maintenance (upgrades, backups, rotation): [`deploy/komodo/README.md`](../deploy/komodo/README.md).

App ingress is out of scope. Containers bind `127.0.0.1:1300x` and are reachable on the shared network by app name.

## DOCR username in Komodo

Before every pull, Komodo logs in to DOCR with an image registry account: a username plus a DigitalOcean token. Each stack names its account by username (`registry_account` in the sync file), and Periphery runs `docker login --username <username> --password-stdin registry.digitalocean.com` on the server.

- DigitalOcean documents two logins: the account email with an API token, or the token as both username and password. Whether DOCR accepts any other username, such as the placeholder `exam-platform`, is unverified.
- Never use the token as the username. `registry_account` is plain stack config: anyone with Read on the stack sees it, and the sync file is public.
- Test the username on the server before configuring Komodo:

  ```bash
  read -rs DO_TOKEN # paste the token with registry read
  printf '%s' "$DO_TOKEN" | docker login registry.digitalocean.com --username exam-platform --password-stdin
  docker logout registry.digitalocean.com
  ```

  If DOCR rejects it, use the DigitalOcean account email instead. It ends up in this public repository, so prefer a team address.

- Changing it takes two edits: the account in Komodo (Settings → Providers), and `registry_account` in all four stacks in `deploy/komodo/exam-platform.toml`. Then run the sync.

## Rollback across compose changes

A deploy pairs two inputs from different places: the image digest from the Komodo Variable, and `compose.deploy.yaml` from `main` at the moment of the deploy. A rollback therefore runs the old image with the newest compose file. That works while the compose file changes only in ways the old image tolerates.

- Safe: adding environment variables (an image ignores those it does not read), labels, logging or resource limits; changing the restart policy or a published host port.
- Breaks the old image: renaming or removing a variable it still reads, or changing what one means; changing `LISTEN_ADDR`, the container port, `WEB_DIR` or another path baked into the image; dropping a network alias that another app still calls.

Keep compose changes backward compatible for one release, as with database migrations: add, release, then remove in a later release. Commit them as `fix` or `feat` so they ship with the app's next release. A `chore` change sits on `main` until the app's next deploy, and that may be a rollback.

To roll back across an incompatible compose change:

1. Preferred: revert the compose change on `main`, then deploy `previous`.
2. Emergency: in Komodo, set the stack's Commit to the commit of the release being restored (GitHub tag `<app>-v<version>`), then deploy `previous`. Its `compose.deploy.yaml` matches the image again. Clear the Commit once `main` is fixed. Until then the sync reports the stack as changed, and running the sync reverts it to `main`.

## Later

- Staging: a second GitHub environment and Komodo server, promoting the same digest. wApp bakes `PUBLIC_AUTH_API_URL`, so its image stays per environment until it reads config at runtime.
- desktop-app: build and sign Tauri bundles in `release.yml`'s `desktop` job and attach them to the release.
- DOCR garbage collection for untagged manifests.

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

- Config: one per app, [`release-please/<app>.json`](../release-please), with its manifest `release-please/<app>.manifest.json`. Apps with a Dockerfile ship as images.
- Tags `<app>-v<version>`, changelog `apps/<app>/CHANGELOG.md`. Each manifest starts its app at `0.0.0` (unreleased), so its first release is `0.1.0`.
- `feat` bumps minor, `fix`/`perf`/`revert` patch, breaking changes minor while below 1.0. `chore`, `ci`, `docs`, `refactor`, `test` and `build` release nothing: a dependency bump that should ship is `fix(deps): …`.

## Release roots

release-please gives a package the commits that touch files under its path. Each app's config holds a single package at the repo root (`.`), so it sees every commit, minus its `exclude-paths`: the other apps, and the directories no build reads (`.github`, `.vscode`, `databases`, `deploy`, `docs`, `release-please`). The changelog, tag and version files still point into `apps/<app>`.

- A change outside the excluded directories releases the app. A `libs/runtime` fix releases aAPI, cAPI, eDd and dApp; wApp also excludes `libs`.
- release-please excludes directories, not files. A commit that touches only root files, such as `Cargo.lock`, `bun.lock` or `package.json`, releases every app. Over-releasing redeploys an unchanged app; under-releasing would leave a fix unshipped.
- A new app needs its own config and manifest, a step in `release.yml`, and an entry in the other apps' `exclude-paths`. A new top-level directory counts toward every app until each config excludes it.

## CI

[`ci.yml`](../.github/workflows/ci.yml) runs on PRs and on `main`; path filters pick the jobs, and `CI passed` is the single check to require.

- Rust runs on any stable toolchain. `rust-toolchain.toml` names the `stable` channel, not a version, so rustup uses the stable toolchain already installed: locally, yours; in CI, the runner image's, never a newer one. Images build on `rust:1`, the newest stable. `rust-version = "1.95"` in both workspaces is the floor they all meet.
- Rust: `cargo fmt`, Clippy (workspace, and `runtime` without `server` as dApp builds it), tests.
- Desktop Rust: the same for dApp's own workspace.
- Bun: frozen install, `bun run check`, `bun run build`.
- Docker: builds each image whose inputs changed, without pushing: the app's directory, plus what its Dockerfile copies from outside it (`Cargo.toml`, `Cargo.lock`, `libs/`, `package.json`, `bun.lock`, `.dockerignore`). `main` writes the layer cache that PRs read.

## Release

[`release.yml`](../.github/workflows/release.yml) on every push to `main`:

1. release-please runs once per app config. It opens or updates the app's release PR, and creates the GitHub release when that PR was just merged. Releases made with `GITHUB_TOKEN` trigger no other workflow, so the same run continues. One config failing still lets the other apps' releases build; the job fails afterwards.
2. Per released image app, [`release-container.yml`](../.github/workflows/release-container.yml) checks out the release commit and builds the existing repo-root Docker context. It pushes `registry.digitalocean.com/<DOCR_REGISTRY>/<app>:<version>` and `:sha-<commit>`, attests build provenance, and appends `**Image:** <repository>@sha256:…` to the release notes.
3. [`deploy.yml`](../.github/workflows/deploy.yml) waits for `production` approval, then runs Komodo's deploy Action with that digest. Deploys of one app run one at a time; a newer pending deploy supersedes an older one.

## Deploy

Production is one DigitalOcean Droplet running Komodo, Caddy and the app stacks ([`deploy/README.md`](../deploy/README.md)). [`deploy/exam-platform.toml`](../deploy/exam-platform.toml) is Komodo's Resource Sync:

- Stacks `exam-platform-<app>` on server `exam-platform`. Each runs `apps/<app>/compose.yaml` from `main`, the same file developers run locally, with `IMAGE` set to the release digest. Komodo pulls the image first, and `--no-build` keeps compose from building `main` instead.
- All stacks join network `platform`, which `pre_deploy` creates if missing. Caddy reaches the public apps there by alias.
- `compose up --wait --wait-timeout 120`: a container that does not turn healthy fails the deploy.
- Action `exam-platform-deploy` sets `EXAM_PLATFORM_<APP>_IMAGE` and deploys the stack. On success it keeps the replaced digest in `EXAM_PLATFORM_<APP>_IMAGE_PREVIOUS`. On failure it redeploys the replaced digest, and the workflow still fails.
- Only admins can write Komodo Variables. The Action runs as Komodo's admin Action user, so the CI key holds Execute on that Action and nothing else (user group `exam-platform-ci`).
- Runtime secrets are Komodo secret Variables, interpolated into stack environments at deploy. They never reach images or this repository.
- Procedure `exam-platform-prune-images` removes unused images every Sunday. A rollback pulls its image from DOCR again.

Rollback:

- Actions → Deploy → Run workflow, with the app and `previous`, or with any digest from a release's **Image** line. `previous` swaps current and previous.
- Without GitHub: set `EXAM_PLATFORM_<APP>_IMAGE` in Komodo, then deploy the stack.
- Stacks read `compose.yaml` from `main`, so a rollback runs the old image with the current compose file: see [Rollback across compose changes](#rollback-across-compose-changes).

## Setup

DigitalOcean: a container registry (`doctl registry create <name>`), then the Droplet and Komodo's first-time setup: [`deploy/README.md`](../deploy/README.md).

GitHub:

- Settings → Actions → General: allow GitHub Actions to create and approve pull requests (release-please).
- Environment `production`: required reviewers, deployment branch `main`.
  - Secrets `KOMODO_API_KEY` and `KOMODO_API_SECRET`: the API key of Komodo service user `exam-platform-ci`.
  - Variable `KOMODO_URL`: `https://exam-platform.freecodecamp.dev`.
- Repository secret `DIGITALOCEAN_ACCESS_TOKEN`: DigitalOcean token with registry read/write.
- Repository variables:
  - `DOCR_REGISTRY`: the registry name.
  - `PUBLIC_AUTH_API_URL`: the aAPI origin wApp's image bakes in.
- Branch protection: require `CI passed`. Release PRs opened with `GITHUB_TOKEN` trigger no CI, so either allow merging them without the check, or give release-please a GitHub App token.

The secrets and variables, with the GitHub CLI:

```bash
gh secret set KOMODO_API_KEY --env production
gh secret set KOMODO_API_SECRET --env production
gh variable set KOMODO_URL --env production --body https://exam-platform.freecodecamp.dev
gh secret set DIGITALOCEAN_ACCESS_TOKEN
gh variable set DOCR_REGISTRY --body <registry>
gh variable set PUBLIC_AUTH_API_URL --body <aAPI origin>
```

## DOCR username in Komodo

Before every pull, Komodo logs in to DOCR with an image registry account: a username plus a DigitalOcean token. Each stack names its account by username (`registry_account` in the sync file), and Periphery runs `docker login --username <username> --password-stdin registry.digitalocean.com` on the server.

- DigitalOcean documents two logins: the account email with an API token, or the token as both username and password. Whether DOCR accepts any other username, such as the placeholder `exam-platform`, is unverified.
- Never use the token as the username. `registry_account` is plain stack config: anyone with Read on the stack sees it, and the sync file is public.
- Test the username on the Droplet before configuring Komodo:

  ```bash
  read -rs DO_TOKEN # paste the token with registry read
  printf '%s' "$DO_TOKEN" | docker login registry.digitalocean.com --username exam-platform --password-stdin
  docker logout registry.digitalocean.com
  ```

  If DOCR rejects it, use the DigitalOcean account email instead. It ends up in this public repository, so prefer a team address.

- Changing it takes two edits: the account in Komodo (Settings → Providers), and `registry_account` in all four stacks in `deploy/exam-platform.toml`. Then run the sync.

## Rollback across compose changes

A deploy pairs two inputs from different places: the image digest from the Komodo Variable, and `compose.yaml` from `main` at the moment of the deploy. A rollback therefore runs the old image with the newest compose file. That works while the compose file changes only in ways the old image tolerates.

- Safe: adding environment variables (an image ignores those it does not read), labels, logging or resource limits; changing the restart policy or a published host port.
- Breaks the old image: renaming or removing a variable it still reads, or changing what one means; changing `LISTEN_ADDR`, the container port, `WEB_DIR` or another path baked into the image; dropping a network alias that another app, or Caddy, still calls.

Keep compose changes backward compatible for one release, as with database migrations: add, release, then remove in a later release. Commit them as `fix` or `feat` so they ship with the app's next release. A `chore` change sits on `main` until the app's next deploy, and that may be a rollback.

To roll back across an incompatible compose change:

1. Preferred: revert the compose change on `main`, then deploy `previous`.
2. Emergency: in Komodo, set the stack's Commit to the commit of the release being restored (GitHub tag `<app>-v<version>`), then deploy `previous`. Its `compose.yaml` matches the image again. Clear the Commit once `main` is fixed. Until then the sync reports the stack as changed, and running the sync reverts it to `main`.

## Later

- Staging: a second GitHub environment and Komodo server, promoting the same digest. wApp bakes `PUBLIC_AUTH_API_URL`, so its image stays per environment until it reads config at runtime.
- Public names for aAPI and wApp: a Caddy site block each ([`deploy/README.md`](../deploy/README.md#routing)).
- desktop-app: build and sign Tauri bundles in `release.yml`'s `desktop` job and attach them to the release.
- DOCR garbage collection for untagged manifests.

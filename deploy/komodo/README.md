# Komodo

Komodo deploys the app stacks ([`docs/releases.md`](../../docs/releases.md)). It runs on the same VM as they do: MongoDB, Core, Periphery, and Caddy for TLS, all from [`compose.yaml`](compose.yaml).

|           |                                                                                                                                                       |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Public    | `https://<KOMODO_DOMAIN>`, serving only `POST /execute/RunAction` and `POST /read/GetUpdate`, the calls `deploy.yml` makes ([`Caddyfile`](Caddyfile)) |
| UI        | SSH tunnel to `127.0.0.1:9120`                                                                                                                        |
| Resources | [`exam-platform.toml`](exam-platform.toml), a Resource Sync: stacks, deploy Action, prune Procedure, CI user group                                    |
| Server    | `exam-platform`: the Periphery in this compose project, connected to Core over the shared `keys` volume                                               |
| On disk   | Stacks in `/etc/komodo/stacks`, database backups in `/etc/komodo/backups`, MongoDB in volume `komodo_mongo-data`                                      |

## Install

The VM needs Docker Engine with the Compose plugin ([install docs](https://docs.docker.com/engine/install/)), a DNS record for the Komodo domain, and inbound 22, 80 and 443 only. Run these as a user in the `docker` group:

```bash
sudo install -d -o "$USER" /opt/exam-platform
git clone https://github.com/freeCodeCamp/exam-platform.git /opt/exam-platform
cd /opt/exam-platform/deploy/komodo
install -m 600 .env.example .env
$EDITOR .env          # domain, and each secret from `openssl rand -hex 32`
docker compose up -d
docker compose ps     # four services running; `docker compose logs caddy` shows the certificate issued
```

## First-time setup

1. Open the UI through a tunnel: `ssh -L 9120:127.0.0.1:9120 <vm>`, then http://localhost:9120. Sign up: the first account becomes super admin. Accounts created later stay disabled until an admin enables them.
2. Check that server `exam-platform` reports OK. Its Periphery connects on start.
3. Settings → Providers: add an image registry account for `registry.digitalocean.com` with a DigitalOcean token scoped to registry read. Test the username first: [DOCR username in Komodo](../../docs/releases.md#docr-username-in-komodo).
4. Settings → Variables: create secret Variables `EXAM_PLATFORM_AUTH_API_SENTRY_DSN`, `EXAM_PLATFORM_CURRICULUM_API_SENTRY_DSN` and `EXAM_PLATFORM_EXAMINER_DASHBOARD_SENTRY_DSN`.
5. Settings → Users: create service user `exam-platform-ci` and an API key for it. In GitHub, environment `production`:
   - Secrets `KOMODO_API_KEY` and `KOMODO_API_SECRET`.
   - Variable `KOMODO_URL` = `https://<KOMODO_DOMAIN>`.
6. Syncs → new Resource Sync:
   - Repository `freeCodeCamp/exam-platform`, branch `main`, resource path `deploy/komodo/exam-platform.toml`.
   - Include user groups on, include variables off, delete off.
   - Execute it. Later edits to the file apply when an admin executes the sync again; it lists pending changes.

The first deploy of each app creates its `EXAM_PLATFORM_<APP>_IMAGE` Variables.

## Maintain

Komodo is not in the deploy path of running apps. Docker restarts the app containers, so they keep serving while Komodo is down or upgrading. Only deploys stop: the GitHub Deploy job fails at `RunAction` and can be re-run once Komodo is back.

Upgrade:

- Komodo: read the [release notes](https://github.com/moghtech/komodo/releases), then bump the `komodo-core` and `komodo-periphery` tags together in `compose.yaml` through a PR.
- MongoDB stays on `8.0` patch releases. A major upgrade waits for Komodo to ship a driver tested against it, then needs MongoDB's `featureCompatibilityVersion` steps.
- Caddy: bump its tag in a PR.
- Then, on the VM:

  ```bash
  cd /opt/exam-platform && git pull
  cd deploy/komodo && docker compose pull && docker compose up -d
  ```

Backups:

- Komodo's default **Backup Core Database** Procedure dumps the database daily at 01:00 UTC to `/etc/komodo/backups`, and keeps the newest 14.
- The dumps are unencrypted and stay on the VM. Keep VM backups or snapshots, or copy the folder elsewhere encrypted.
- Restore into an empty database: stop Core and Periphery, drop the database, restore, then start them again.

  ```bash
  cd /opt/exam-platform/deploy/komodo && set -a && . ./.env && set +a
  docker compose stop core periphery
  docker compose exec mongo mongosh --quiet -u "$KOMODO_DATABASE_USERNAME" -p "$KOMODO_DATABASE_PASSWORD" \
    --authenticationDatabase admin --eval 'db.getSiblingDB("komodo").dropDatabase()'
  docker run --rm --network komodo_default -v /etc/komodo/backups:/backups \
    -e KOMODO_CLI_DATABASE_TARGET_ADDRESS=mongo:27017 \
    -e KOMODO_CLI_DATABASE_TARGET_USERNAME="$KOMODO_DATABASE_USERNAME" \
    -e KOMODO_CLI_DATABASE_TARGET_PASSWORD="$KOMODO_DATABASE_PASSWORD" \
    -e KOMODO_CLI_DATABASE_TARGET_DB_NAME=komodo \
    ghcr.io/moghtech/komodo-cli:2.3.3 km database restore -y # newest; or --restore-folder <dated folder>
  docker compose start core periphery
  ```

Disk:

- The `exam-platform-prune-images` Procedure removes unused images every Sunday at 03:00 UTC.
- Watch `/etc/komodo/backups` and Docker volumes too.

Rotate:

- CI key: create a new API key for `exam-platform-ci`, replace both GitHub secrets, then delete the old key.
- DOCR token: update the registry account under Settings → Providers.
- App secrets: update the secret Variable, then redeploy the stack. Variables apply at deploy.
- `KOMODO_JWT_SECRET`: edit `.env`, then `docker compose up -d core`. Everyone is logged out.
- MongoDB password: change it with `mongosh` (`db.changeUserPassword`), then edit `.env` and run `docker compose up -d core`. The init variables only apply to an empty volume.

Logs: `docker compose logs -f core periphery caddy`.

Do not:

- Manage this compose project from Komodo.
- Publish port 9120 or MongoDB.
- Expose `/ws/periphery`: no remote servers connect.

To open the UI publicly instead of tunnelling, add a `handle` to the Caddyfile that allows trusted IPs only, or enable OIDC (`KOMODO_OIDC_*`).

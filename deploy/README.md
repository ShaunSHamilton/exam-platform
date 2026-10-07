# Deploy

Production is one DigitalOcean Droplet. Komodo deploys the app stacks on it ([`docs/releases.md`](../docs/releases.md)). Komodo itself (MongoDB, Core, Periphery) and Caddy run beside them from [`compose.yaml`](compose.yaml), started by [`bootstrap.sh`](bootstrap.sh).

|                                               |                                                                                                                        |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `https://exam-platform.freecodecamp.dev`      | Komodo: the UI, behind login and 2FA, and the API `deploy.yml` calls                                                   |
| `https://examiner-dashboard.freecodecamp.dev` | eDd                                                                                                                    |
| Resources                                     | [`exam-platform.toml`](exam-platform.toml), a Resource Sync: stacks, deploy Action, prune Procedure, CI user group     |
| Server                                        | `exam-platform`: the Periphery in this compose project. It connects to Core with keys from the shared `keys` volume    |
| On disk                                       | Repository and `deploy/.env` in `/opt/exam-platform`, stacks in `/etc/komodo/stacks`, backups in `/etc/komodo/backups` |

## Create the Droplet

With [`doctl`](https://docs.digitalocean.com/reference/doctl/how-to/install/) after `doctl auth init`, or the same values in the control panel:

```bash
doctl compute ssh-key list # the ID of your key
doctl compute droplet create exam-platform --image ubuntu-26-04-x64 --size s-2vcpu-4gb \
  --region <region> --ssh-keys <key-id> --enable-backups --enable-monitoring \
  --tag-names exam-platform --wait
doctl compute reserved-ip create --droplet-id <droplet-id> # keeps the address across rebuilds
doctl compute firewall create --name exam-platform --tag-names exam-platform \
  --inbound-rules "protocol:tcp,ports:22,address:0.0.0.0/0,address:::/0 protocol:tcp,ports:80,address:0.0.0.0/0,address:::/0 protocol:tcp,ports:443,address:0.0.0.0/0,address:::/0 protocol:udp,ports:443,address:0.0.0.0/0,address:::/0" \
  --outbound-rules "protocol:tcp,ports:all,address:0.0.0.0/0,address:::/0 protocol:udp,ports:all,address:0.0.0.0/0,address:::/0 protocol:icmp,address:0.0.0.0/0,address:::/0"
```

- Region: the registry's, so pulls stay in one data center. Size: images build in GitHub Actions, not here; 4 GB holds Komodo, the four apps and room to grow.
- DNS in `freecodecamp.dev`: A records `exam-platform` and `examiner-dashboard` → the reserved IP. DNS only, not proxied: Caddy answers Let's Encrypt's challenges itself.

## Install

```bash
ssh root@<reserved-ip>
git clone https://github.com/freeCodeCamp/exam-platform.git /opt/exam-platform
/opt/exam-platform/deploy/bootstrap.sh
```

`bootstrap.sh`:

1. Installs Docker Engine and Compose from Docker's apt repository, with log rotation and `live-restore`.
2. Creates the apps' network `platform`.
3. Writes `deploy/.env` from [`.env.example`](.env.example) and generates its secrets, including Komodo's first admin password.
4. Pulls and starts `compose.yaml`, then reloads Caddy.

It prints the Komodo URL and where to read the admin password. Create the DNS records first: Caddy retries until the names resolve, then issues both certificates. As Droplet user data (`--user-data-file`), a `#!/bin/bash` script with the `git clone` and `bootstrap.sh` lines does the same on first boot; cloud-init logs to `/var/log/cloud-init-output.log`.

## First-time setup

1. Open `https://exam-platform.freecodecamp.dev` and log in as `admin` with the password from `.env`. Change the password, then add a passkey or TOTP. Nobody can sign up: admins add users under Settings → Users, and each user adds 2FA at first login.
2. Check that server `exam-platform` reports OK. Its Periphery connects on start.
3. Settings → Providers: add an image registry account for `registry.digitalocean.com` with a DigitalOcean token scoped to registry read. Test the username first: [DOCR username in Komodo](../docs/releases.md#docr-username-in-komodo).
4. Settings → Variables: create secret Variables `EXAM_PLATFORM_AUTH_API_SENTRY_DSN`, `EXAM_PLATFORM_CURRICULUM_API_SENTRY_DSN` and `EXAM_PLATFORM_EXAMINER_DASHBOARD_SENTRY_DSN`.
5. Settings → Users: create service user `exam-platform-ci` and an API key for it. Store the key in GitHub: [Setup](../docs/releases.md#setup).
6. Syncs → new Resource Sync:
   - Repository `freeCodeCamp/exam-platform`, branch `main`, resource path `deploy/exam-platform.toml`.
   - Include user groups on, include variables off, delete off.
   - Execute it. Later edits to the file apply when an admin executes the sync again; it lists pending changes.

The first deploy of each app creates its `EXAM_PLATFORM_<APP>_IMAGE` Variables. Until eDd's first deploy, its name answers 502.

## Routing

Both names resolve to the Droplet. Caddy ([`caddy/Caddyfile`](caddy/Caddyfile)) holds a certificate for each, redirects HTTP to HTTPS, and picks the upstream by host name:

| Host                                  | Upstream                                       | Over                                                       |
| ------------------------------------- | ---------------------------------------------- | ---------------------------------------------------------- |
| `exam-platform.freecodecamp.dev`      | `core:9120`, except `/ws/periphery` (404)      | the compose project's default network                      |
| `examiner-dashboard.freecodecamp.dev` | `examiner-dashboard:13003`, its alias and port | network `platform`, which every app's `compose.yaml` joins |

Apps also bind `127.0.0.1:1300x` on the Droplet, for debugging over SSH. Only Caddy is public.

To make another app public, e.g. aAPI:

1. Add `AUTH_API_DOMAIN=…` to `.env.example`, and pass it in the caddy service's `environment` in `compose.yaml`.
2. Add a site block to `caddy/Caddyfile`: `{$AUTH_API_DOMAIN} { reverse_proxy auth-api:13001 }`.
3. Create the DNS record, merge, then re-run `bootstrap.sh`: it adds the new key to `.env` and reloads Caddy.

## Maintain

Komodo is not in the request path. Docker restarts the app containers, so they keep serving while Komodo is down or upgrading. Only deploys stop: the GitHub Deploy job fails at `RunAction` and can be re-run once Komodo is back. Caddy is in the path: a Caddyfile change reloads without dropping connections, a new Caddy image restarts it.

Update: merge to `main`, then run `/opt/exam-platform/deploy/bootstrap.sh` on the Droplet. It pulls `main`, fills new `.env` keys, applies `compose.yaml` and reloads the Caddyfile.

Upgrade:

- Komodo: read the [release notes](https://github.com/moghtech/komodo/releases), then bump the `komodo-core` and `komodo-periphery` tags together in `compose.yaml` through a PR.
- MongoDB stays on `8.0`: each update pulls its newest patch release. A major upgrade waits for Komodo to ship a driver tested against it, then needs MongoDB's `featureCompatibilityVersion` steps.
- Caddy: bump its tag in a PR.
- Ubuntu installs security updates by itself. Reboot when `/var/run/reboot-required` exists; every container restarts with Docker.

Backups:

- Droplet backups (`--enable-backups`) image the whole disk weekly.
- Komodo's default **Backup Core Database** Procedure dumps the database daily at 01:00 UTC to `/etc/komodo/backups`, and keeps the newest 14. The dumps are unencrypted and on the Droplet: copy them elsewhere, encrypted, if the Droplet backups are not enough.
- Restore into an empty database: stop Core and Periphery, drop the database, restore, then start them again.

  ```bash
  cd /opt/exam-platform/deploy && set -a && . ./.env && set +a
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
- Container logs rotate (Docker's `local` driver). Watch `/etc/komodo/backups` and `docker system df`.

Rotate:

- CI key: create a new API key for `exam-platform-ci`, replace both GitHub secrets, then delete the old key.
- DOCR token: update the registry account under Settings → Providers.
- App secrets: update the secret Variable, then redeploy the stack. Variables apply at deploy.
- Login passwords: in the UI. `KOMODO_INIT_ADMIN_PASSWORD` only applies to an empty database.
- `KOMODO_JWT_SECRET`: edit `.env`, then `docker compose up -d core`. Everyone is logged out.
- MongoDB password: change it with `mongosh` (`db.changeUserPassword`), then edit `.env` and run `docker compose up -d core`. The init variables only apply to an empty volume.

Logs: `docker compose logs -f core periphery caddy`, from `/opt/exam-platform/deploy`.

Do not:

- Manage this compose project from Komodo.
- Publish MongoDB, or port 9120 beyond `127.0.0.1`.
- Remove the `/ws/periphery` block: no remote servers connect.

To log in with GitHub or an OIDC provider instead of passwords, set `KOMODO_GITHUB_OAUTH_*` or `KOMODO_OIDC_*` on the core service. Sign-up stays closed, so users link the provider to an account an admin created.

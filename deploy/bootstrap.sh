#!/usr/bin/env bash
# Sets up, or updates, the exam platform Droplet: Docker, this repository in /opt/exam-platform,
# the shared `platform` network, deploy/.env with generated secrets, then compose.yaml (Komodo
# and Caddy). Ubuntu 26.04, as root. Re-run it to update: it fast-forwards main and only fills
# secrets that are still empty.
#
#   git clone https://github.com/ShaunSHamilton/exam-platform.git /opt/exam-platform
#   /opt/exam-platform/deploy/bootstrap.sh
set -euo pipefail

REPO=https://github.com/ShaunSHamilton/exam-platform.git
DIR=/opt/exam-platform
SECRETS=(KOMODO_INIT_ADMIN_PASSWORD KOMODO_DATABASE_PASSWORD KOMODO_JWT_SECRET KOMODO_WEBHOOK_SECRET)

# A function, so bash has read all of it before `git pull` rewrites this file.
main() {
  if [[ $EUID -ne 0 ]]; then
    echo "Run as root." >&2
    exit 1
  fi
  # cloud-init runs user data without HOME; git and docker need one. apt must never prompt.
  export HOME=${HOME:-/root} DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a

  # Pull first, then hand over to the pulled copy so new steps apply on this run.
  if [[ ${BOOTSTRAP_SYNCED:-} != 1 ]]; then
    sync_repo
    BOOTSTRAP_SYNCED=1 exec bash "$DIR/deploy/bootstrap.sh" "$@"
  fi

  install_docker
  configure_docker
  docker network inspect platform >/dev/null 2>&1 || docker network create platform >/dev/null
  write_env
  start
  summary
}

sync_repo() {
  command -v git >/dev/null || { apt-get update && apt-get install -y git; }
  if [[ -d $DIR/.git ]]; then
    git -C "$DIR" pull --ff-only
  else
    git clone "$REPO" "$DIR"
  fi
}

# Docker Engine and the Compose plugin from Docker's apt repository:
# https://docs.docker.com/engine/install/ubuntu/
install_docker() {
  if command -v docker >/dev/null && docker compose version >/dev/null 2>&1; then
    return
  fi
  apt-get update
  apt-get install -y ca-certificates curl
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
  chmod a+r /etc/apt/keyrings/docker.asc
  cat >/etc/apt/sources.list.d/docker.sources <<EOF
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}")
Components: stable
Architectures: $(dpkg --print-architecture)
Signed-By: /etc/apt/keyrings/docker.asc
EOF
  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
}

# The default json-file log driver never rotates, and would fill the disk. live-restore keeps
# containers running while Docker restarts for an upgrade.
configure_docker() {
  if [[ -f /etc/docker/daemon.json ]]; then
    return
  fi
  printf '{\n  "log-driver": "local",\n  "live-restore": true\n}\n' >/etc/docker/daemon.json
  systemctl restart docker
}

write_env() {
  local env=$DIR/deploy/.env line key
  if [[ ! -f $env ]]; then
    install -m 600 "$DIR/deploy/.env.example" "$env"
  fi
  chmod 600 "$env"
  # Keys .env.example gained since .env was written.
  while IFS= read -r line; do
    if [[ $line =~ ^([A-Z0-9_]+)= ]] && ! grep -q "^${BASH_REMATCH[1]}=" "$env"; then
      printf '%s\n' "$line" >>"$env"
    fi
  done <"$DIR/deploy/.env.example"
  for key in "${SECRETS[@]}"; do
    sed -i "s/^$key=\$/$key=$(openssl rand -hex 32)/" "$env"
  done
}

start() {
  cd "$DIR/deploy"
  docker compose pull --quiet
  docker compose up --detach --remove-orphans --wait --wait-timeout 300
  # A running Caddy keeps its config until told: applies Caddyfile changes without dropping
  # connections, and fails here, keeping the old config, if the new one is invalid.
  docker compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
}

summary() {
  local domain user
  domain=$(sed -n 's/^KOMODO_DOMAIN=//p' "$DIR/deploy/.env")
  user=$(sed -n 's/^KOMODO_INIT_ADMIN_USERNAME=//p' "$DIR/deploy/.env")
  cat <<EOF

Komodo: https://$domain
First login: user $user, password from
  sed -n 's/^KOMODO_INIT_ADMIN_PASSWORD=//p' $DIR/deploy/.env
It only applies to the first start. Next: first-time setup in $DIR/deploy/README.md.
EOF
}

main "$@"

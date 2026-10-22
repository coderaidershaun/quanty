#!/usr/bin/env bash
set -euo pipefail
umask 077

QDRANT_VERSION=1.20.0
FALKORDB_VERSION=6.0.3
NODE_VERSION=24.22.1
RUST_VERSION=1.97.1
TRUNK_VERSION=0.22.0
CODEX_VERSION=0.163.0
CLAUDE_VERSION=2.1.308
QUANTY_USER=quanty
QUANTY_HOME=/srv/quanty
QUANTY_DOMAIN=${1:?Usage: sudo deploy/install-ubuntu.sh quanty.example.com}
SOURCE_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
DEPLOY_DIR="$SOURCE_DIR/deploy"

[[ $EUID == 0 ]] || { echo 'Run this script with sudo.' >&2; exit 1; }
[[ $(uname -m) == x86_64 ]] || { echo 'This installer supports Ubuntu x86_64 only.' >&2; exit 1; }
. /etc/os-release
[[ $ID == ubuntu && $VERSION_ID == 24.04 ]] || { echo 'This installer requires Ubuntu 24.04 LTS.' >&2; exit 1; }
[[ $QUANTY_DOMAIN =~ ^[a-zA-Z0-9]([a-zA-Z0-9.-]*[a-zA-Z0-9])?$ && $QUANTY_DOMAIN == *.* ]] || { echo 'Give a DNS hostname.' >&2; exit 1; }

apt-get update
apt-get install -y ca-certificates curl gpg lsb-release openssl git build-essential pkg-config libssl-dev libgomp1 poppler-utils python3 python3-dotenv xz-utils debian-keyring debian-archive-keyring apt-transport-https
id "$QUANTY_USER" >/dev/null 2>&1 || useradd --system --user-group --create-home --home-dir "$QUANTY_HOME" --shell /bin/bash "$QUANTY_USER"
[[ $(getent passwd "$QUANTY_USER" | cut -d: -f6) == "$QUANTY_HOME" ]] || { echo 'quanty must have /srv/quanty as its account home.' >&2; exit 1; }
for folder in "$QUANTY_HOME" "$QUANTY_HOME/content" "$QUANTY_HOME/data" "$QUANTY_HOME/data/qdrant" "$QUANTY_HOME/data/uploads" "$QUANTY_HOME/data/exports"; do
    [[ -d $folder ]] || install -d -m 0750 -o "$QUANTY_USER" -g "$QUANTY_USER" "$folder"
done
install -d -m 0755 /etc/quanty /opt/falkordb /etc/redis/quanty /etc/systemd/system/redis-server.service.d
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT

install_new() {
    local source=$1 destination=$2 mode=${3:-0644}
    [[ -e $destination ]] || install -m "$mode" "$source" "$destination"
}

if [[ ! -e /usr/share/keyrings/redis-archive-keyring.gpg ]]; then
    curl -fsSL https://packages.redis.io/gpg -o "$scratch/redis.gpg"
    gpg --dearmor --output /usr/share/keyrings/redis-archive-keyring.gpg "$scratch/redis.gpg"
    chmod 0644 /usr/share/keyrings/redis-archive-keyring.gpg
fi
if [[ ! -e /etc/apt/sources.list.d/redis.list ]]; then
    printf 'deb [signed-by=/usr/share/keyrings/redis-archive-keyring.gpg] https://packages.redis.io/deb %s main\n' "$(lsb_release -cs)" > /etc/apt/sources.list.d/redis.list
    chmod 0644 /etc/apt/sources.list.d/redis.list
fi
if [[ ! -e /usr/share/keyrings/caddy-stable-archive-keyring.gpg ]]; then
    curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key -o "$scratch/caddy.gpg"
    gpg --dearmor --output /usr/share/keyrings/caddy-stable-archive-keyring.gpg "$scratch/caddy.gpg"
    chmod 0644 /usr/share/keyrings/caddy-stable-archive-keyring.gpg
fi
if [[ ! -e /etc/apt/sources.list.d/caddy-stable.list ]]; then
    curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt -o /etc/apt/sources.list.d/caddy-stable.list
    chmod 0644 /etc/apt/sources.list.d/caddy-stable.list
fi
apt-get update
caddy_config_existed=false
[[ ! -e /etc/caddy/Caddyfile ]] || caddy_config_existed=true
apt-get install -y redis caddy
redis_major=$(redis-server --version | sed -n 's/.*v=\([0-9]*\).*/\1/p')
[[ -n $redis_major && $redis_major -ge 8 ]] || { echo 'FalkorDB requires Redis 8 or newer.' >&2; exit 1; }
[[ -d $QUANTY_HOME/data/falkordb ]] || install -d -m 0750 -o redis -g redis "$QUANTY_HOME/data/falkordb"
# Redis needs to cross the library root without reading its secrets.
command -v setfacl >/dev/null || apt-get install -y acl
setfacl -m u:redis:--x "$QUANTY_HOME" "$QUANTY_HOME/data"

if [[ ! -e $QUANTY_HOME/.env ]]; then
    qdrant_key=$(openssl rand -hex 32)
    falkordb_password=$(openssl rand -hex 32)
    login_password=$(openssl rand -hex 32)
    mcp_token=$(openssl rand -hex 32)
    session_key=$(openssl rand -hex 32)
    cat > "$scratch/quanty.env" <<ENV
QUANTY_LISTEN=127.0.0.1:8080
QUANTY_PASSWORD=$login_password
QUANTY_MCP_TOKEN=$mcp_token
QUANTY_SESSION_KEY=$session_key
QUANTY_UPLOAD_LIMIT_MB=1024
QDRANT_URL=http://127.0.0.1:6334
QDRANT_API_KEY=$qdrant_key
FALKORDB_URL=falkor://127.0.0.1:6379
FALKORDB_PASSWORD=$falkordb_password
FALKORDB_GRAPH=quanty
CONTENT_DIR=/srv/quanty/content
EMBEDDING_GEMINI_API_KEY=
CONVERTER_JEV_API_KEY=
LLM_PROVIDER=codex
LLM_MODEL=gpt-6.1-sol
LLM_THINKING=medium
ENV
    install -m 0600 -o "$QUANTY_USER" -g "$QUANTY_USER" "$scratch/quanty.env" "$QUANTY_HOME/.env"
fi
/usr/bin/python3 - "$QUANTY_HOME/.env" "$DEPLOY_DIR/redis-falkordb.conf" /etc/redis/quanty/falkordb.conf <<'PY'
import grp, os, sys
from pathlib import Path
from dotenv import dotenv_values
values = dotenv_values(sys.argv[1], interpolate=False)
for name in ('QDRANT_API_KEY', 'FALKORDB_PASSWORD', 'QUANTY_PASSWORD', 'QUANTY_SESSION_KEY'):
    if not values.get(name):
        raise SystemExit(f'Set {name} in {sys.argv[1]} before installing services.')
target = Path(sys.argv[3])
if not target.exists():
    password = values['FALKORDB_PASSWORD']
    if any(ord(char) < 32 or ord(char) > 126 for char in password):
        raise SystemExit('FALKORDB_PASSWORD must contain printable ASCII only.')
    password = password.replace('\\', '\\\\').replace('"', '\\"')
    with target.open('x') as file:
        file.write(Path(sys.argv[2]).read_text().replace('REPLACE_WITH_FALKORDB_PASSWORD', password))
    os.chmod(target, 0o640)
    os.chown(target, 0, grp.getgrnam("redis").gr_gid)
PY
if ! grep -Fxq 'include /etc/redis/quanty/falkordb.conf' /etc/redis/redis.conf; then
    printf '\ninclude /etc/redis/quanty/falkordb.conf\n' >> /etc/redis/redis.conf
fi
install_new "$DEPLOY_DIR/redis-falkordb.service.conf" /etc/systemd/system/redis-server.service.d/quanty.conf
install_new "$DEPLOY_DIR/qdrant-config.yaml" /etc/quanty/qdrant-config.yaml
install_new "$DEPLOY_DIR/qdrant.service" /etc/systemd/system/qdrant.service
install_new "$DEPLOY_DIR/quanty.service" /etc/systemd/system/quanty.service

if [[ ! -x /usr/local/bin/qdrant ]]; then
    curl -fL "https://github.com/qdrant/qdrant/releases/download/v${QDRANT_VERSION}/qdrant-x86_64-unknown-linux-gnu.tar.gz" -o "$scratch/qdrant.tar.gz"
    tar -xzf "$scratch/qdrant.tar.gz" -C "$scratch"
    install -m 0755 "$scratch/qdrant" /usr/local/bin/qdrant
fi
if [[ ! -e /opt/falkordb/falkordb.so ]]; then
    base="https://github.com/FalkorDB/FalkorDB/releases/download/v${FALKORDB_VERSION}"
    curl -fL "$base/falkordb-x64.so" -o "$scratch/falkordb-x64.so"
    curl -fL "$base/falkordb-x64.so.sha256" -o "$scratch/falkordb-x64.so.sha256"
    (cd "$scratch" && sha256sum -c falkordb-x64.so.sha256)
    install -m 0755 "$scratch/falkordb-x64.so" /opt/falkordb/falkordb.so
fi
if [[ ! -x /opt/node/bin/node ]]; then
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" -o "$scratch/node.tar.xz"
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt" -o "$scratch/SHASUMS256.txt"
    (cd "$scratch" && grep " node-v${NODE_VERSION}-linux-x64.tar.xz\$" SHASUMS256.txt | sed 's|node-v[^ ]*|node.tar.xz|' | sha256sum -c -)
    install -d -m 0755 /opt/node
    tar -xJf "$scratch/node.tar.xz" -C /opt/node --strip-components=1
fi
for binary in node npm npx; do
    [[ -e /usr/local/bin/$binary ]] || ln -s "/opt/node/bin/$binary" "/usr/local/bin/$binary"
done
run_user() {
    runuser -u "$QUANTY_USER" -- env PATH="$QUANTY_HOME/.cargo/bin:$QUANTY_HOME/.local/bin:$QUANTY_HOME/.npm-global/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin" "$@"
}
if [[ ! -x $QUANTY_HOME/.npm-global/bin/codex ]]; then
    run_user npm install --global --prefix "$QUANTY_HOME/.npm-global" "@openai/codex@$CODEX_VERSION"
fi
if [[ ! -x $QUANTY_HOME/.npm-global/bin/claude ]]; then
    run_user npm install --global --prefix "$QUANTY_HOME/.npm-global" "@anthropic-ai/claude-code@$CLAUDE_VERSION"
fi
if [[ ! -x $QUANTY_HOME/.cargo/bin/rustup ]]; then
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$scratch/rustup.sh"
    chmod 0755 "$scratch" "$scratch/rustup.sh"
    run_user sh "$scratch/rustup.sh" -y --default-toolchain "$RUST_VERSION" --profile minimal
fi
run_user rustup target add wasm32-unknown-unknown
if [[ ! -x $QUANTY_HOME/.cargo/bin/trunk ]]; then
    run_user cargo install --locked trunk --version "$TRUNK_VERSION"
fi
run_user test -r "$SOURCE_DIR/Cargo.toml"
run_user bash -ec 'cd "$1/crates/web-ui"; trunk build --release; cd "$1"; cargo build --release -p server' bash "$SOURCE_DIR"
install -m 0755 "$SOURCE_DIR/target/release/quanty" /usr/local/bin/quanty

# The package supplies a welcome configuration; keep any owner configuration.
if [[ $caddy_config_existed == false ]]; then
    sed "s/quanty.example.com/$QUANTY_DOMAIN/" "$DEPLOY_DIR/Caddyfile" > /etc/caddy/Caddyfile
    chmod 0644 /etc/caddy/Caddyfile
else
    sed "s/quanty.example.com/$QUANTY_DOMAIN/" "$DEPLOY_DIR/Caddyfile" > "$scratch/Caddyfile.proposed"
    install_new "$scratch/Caddyfile.proposed" /etc/quanty/Caddyfile.proposed
    echo 'Review /etc/quanty/Caddyfile.proposed and install it as /etc/caddy/Caddyfile before enabling HTTPS.'
fi
caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
systemctl daemon-reload
systemctl enable --now qdrant redis-server
systemctl restart redis-server
systemctl enable --now quanty
systemctl enable --now caddy
systemctl reload caddy
echo "Installed. Add the Gemini/Jev keys to $QUANTY_HOME/.env, sign the CLIs in as $QUANTY_USER, then restart quanty."
echo "Read QUANTY_PASSWORD privately from $QUANTY_HOME/.env and open https://$QUANTY_DOMAIN."

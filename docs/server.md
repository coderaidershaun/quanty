# Run quanty on an Ubuntu server, no Docker

One Ubuntu 24.04 LTS x86_64 host runs quanty, Qdrant, Redis 8 with FalkorDB, and Caddy. People open the app over HTTPS; agents use the same host's `/mcp`. The stores and quanty listen on loopback. Only Caddy's ports 80 and 443 need to be public. Point a DNS A record (and an AAAA record only if IPv6 works) at the host before requesting HTTPS.

The supplied installer targets Ubuntu 24.04 LTS x86_64. The pinned FalkorDB module requires glibc 2.38 and GLIBCXX 3.4.31; Ubuntu 22.04 cannot load it. Other Ubuntu releases and architectures have not been verified. Commands below use a dedicated `quanty` account whose home is `/srv/quanty`. Keep all model sign-ins under that account. The deployment files were checked against official vendor sources on 22 October 2026; the installer has not been executed on Ubuntu as part of this documentation change.

## 1. The account and folders

```bash
sudo useradd --system --user-group --create-home --home-dir /srv/quanty --shell /bin/bash quanty
sudo install -d -o quanty -g quanty -m 0750 /srv/quanty/content /srv/quanty/data
sudo -u quanty git clone https://github.com/coderaidershaun/quanty.git /srv/quanty/src
```

For an existing account, check `getent passwd quanty` instead of running `useradd` again. The repository is separate from the library:

| Path | Holds |
| --- | --- |
| `/srv/quanty/content/` | Converted pages, pictures, insights and source PDFs saved under `_uploads/` |
| `/srv/quanty/data/qdrant/` | Vectors |
| `/srv/quanty/data/falkordb/` | The graph's persisted Redis files |
| `/srv/quanty/data/uploads/` | Temporary PDF uploads; unused folders older than seven days are removed at server start |
| `/srv/quanty/data/exports/` | Temporary downloads; folders older than one day are removed at server start |
| `/srv/quanty/.env` | The one library's settings and secrets; mode `0600`, owned by `quanty` |

The library itself is the three folders `content`, `data/qdrant` and `data/falkordb`. Kept model answers under `data/` and built `datasets/` are useful to retain too.

## 2. Install the services

Read [install-ubuntu.sh](../deploy/install-ubuntu.sh), then run it from the clone:

```bash
cd /srv/quanty/src
sudo deploy/install-ubuntu.sh quanty.example.com
```

Replace the hostname. Version variables are at the top: Qdrant `1.20.0`, FalkorDB `6.0.3`, Node `24.22.1` LTS, Rust `1.97.1`, Trunk `0.22.0`, Codex `0.163.0` and Claude Code `2.1.308`. Redis and Caddy receive the supported packages from their official apt repositories; the script checks that Redis is at least version 8.

Use a dedicated host: installing apt packages and including the FalkorDB configuration changes its Redis service. Repeated runs keep existing `.env`, library folders, CLI sign-ins, binaries and service configuration. They rebuild quanty; they do not upgrade already installed pinned binaries. Review configuration changes and upgrades separately. If Caddy already has a configuration, the script writes `/etc/quanty/Caddyfile.proposed` for you to review and install.

The following sections explain the installer's operations and show checks to run on the server.

### Qdrant

[Qdrant's release](https://github.com/qdrant/qdrant/releases/tag/v1.20.0) supplies the Linux tarball:

```bash
QDRANT_VERSION=1.20.0
curl -fL "https://github.com/qdrant/qdrant/releases/download/v${QDRANT_VERSION}/qdrant-x86_64-unknown-linux-gnu.tar.gz" -o /tmp/qdrant.tar.gz
tar -xzf /tmp/qdrant.tar.gz -C /tmp
sudo install -m 0755 /tmp/qdrant /usr/local/bin/qdrant
```

The installer creates the API key with `openssl rand -hex 32` only when it first creates `.env`. [qdrant-config.yaml](../deploy/qdrant-config.yaml) sets storage to `/srv/quanty/data/qdrant`, disables telemetry and binds to `127.0.0.1`. [qdrant.service](../deploy/qdrant.service) reads `QDRANT_API_KEY` from the same `.env` and passes it to Qdrant as `QDRANT__SERVICE__API_KEY`. Its pre-start check refuses an empty key. YAML does not substitute environment variables.

Qdrant 1.20 requires an HTTP listener, so HTTP `6333` and gRPC `6334` both stay on loopback with the API key. Quanty uses `6334`; the administrative check uses `6333`. See [Qdrant configuration](https://github.com/qdrant/qdrant/blob/v1.20.0/src/settings.rs) and [the listener startup](https://github.com/qdrant/qdrant/blob/v1.20.0/src/main.rs).

```bash
sudo systemctl enable --now qdrant
read -rsp 'Qdrant API key: ' QDRANT_API_KEY; echo
curl --fail -H "api-key: $QDRANT_API_KEY" http://127.0.0.1:6333/collections
unset QDRANT_API_KEY
```

The reply is JSON with the collections; a fresh library has none.

### Redis 8 and FalkorDB

Use [Redis's apt repository](https://redis.io/docs/latest/operate/oss_and_stack/install/install-stack/linux/), since the Ubuntu archive can contain Redis 7:

```bash
sudo apt-get install -y lsb-release curl gpg
curl -fsSL https://packages.redis.io/gpg | sudo gpg --dearmor -o /usr/share/keyrings/redis-archive-keyring.gpg
sudo chmod 644 /usr/share/keyrings/redis-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/redis-archive-keyring.gpg] https://packages.redis.io/deb $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/redis.list
sudo apt-get update
sudo apt-get install -y redis libgomp1
redis-server --version
```

The [FalkorDB release](https://github.com/FalkorDB/FalkorDB/releases/tag/v6.0.3) supplies `falkordb-x64.so` and its SHA-256 file:

```bash
FALKORDB_VERSION=6.0.3
curl -fL "https://github.com/FalkorDB/FalkorDB/releases/download/v${FALKORDB_VERSION}/falkordb-x64.so" -o /tmp/falkordb-x64.so
curl -fL "https://github.com/FalkorDB/FalkorDB/releases/download/v${FALKORDB_VERSION}/falkordb-x64.so.sha256" -o /tmp/falkordb-x64.so.sha256
(cd /tmp && sha256sum -c falkordb-x64.so.sha256)
sudo install -D -m 0755 /tmp/falkordb-x64.so /opt/falkordb/falkordb.so
```

The installer generates `FALKORDB_PASSWORD` with `openssl rand -hex 32` on first creation of `.env`. It renders [redis-falkordb.conf](../deploy/redis-falkordb.conf) to `/etc/redis/quanty/falkordb.conf`, replacing its password placeholder with that value. Redis does not expand environment variables in `requirepass`. The rendered file is `root:redis`, mode `0640`. The installer appends one `include /etc/redis/quanty/falkordb.conf` to `/etc/redis/redis.conf`, preserving the original file's settings. It grants Redis traversal of the library root, owns only `data/falkordb` as `redis`, and adds a systemd writable-path override for that folder.

The drop-in loads the module, requires the password, binds to `127.0.0.1` and puts its data in `/srv/quanty/data/falkordb`. On password rotation, update `.env` and the rendered Redis `requirepass` together, then restart Redis and quanty. Rerunning the installer intentionally does not replace an existing configuration.

```bash
sudo systemctl restart redis-server
read -rsp 'FalkorDB password: ' REDISCLI_AUTH; echo
export REDISCLI_AUTH
redis-cli PING
redis-cli MODULE LIST
unset REDISCLI_AUTH
```

Expect `PONG` and a module named `graph`.

## 3. Model tools and headless sign-in

Poppler supplies `pdftoppm` and `pdfinfo`. The installer installs `poppler-utils` and Node's official LTS tarball. For a manual first installation:

```bash
sudo apt-get install -y poppler-utils xz-utils
NODE_VERSION=24.22.1
curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" -o "/tmp/node-v${NODE_VERSION}-linux-x64.tar.xz"
curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt" -o /tmp/SHASUMS256.txt
(cd /tmp && grep " node-v${NODE_VERSION}-linux-x64.tar.xz$" SHASUMS256.txt | sha256sum -c -)
sudo install -d -m 0755 /opt/node
sudo tar -xJf "/tmp/node-v${NODE_VERSION}-linux-x64.tar.xz" -C /opt/node --strip-components=1
```

The [Codex npm package](https://learn.chatgpt.com/docs/cli) and [Claude Code npm package](https://code.claude.com/docs/en/setup#install-with-npm) go under `/srv/quanty/.npm-global`. Install and sign in as the service account, with Node 22 or newer on `PATH`:

```bash
sudo -iu quanty
export PATH="$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.cargo/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin"
CODEX_VERSION=0.163.0
CLAUDE_VERSION=2.1.308
npm install --global --prefix "$HOME/.npm-global" "@openai/codex@$CODEX_VERSION" "@anthropic-ai/claude-code@$CLAUDE_VERSION"
codex login --device-auth
codex login status
claude setup-token
```

[Codex device authentication](https://learn.chatgpt.com/docs/auth) displays a URL and code to complete in your browser. As an alternative, copy an existing `~/.codex/auth.json` securely into `/srv/quanty/.codex/auth.json` and make it readable only by `quanty`; a login stored only in a desktop keychain must first be made available as a file. Treat that file as a password.

`claude setup-token` prints a long-lived subscription token. Put it in `/srv/quanty/.env` as `CLAUDE_CODE_OAUTH_TOKEN='…'`, or securely copy your authenticated `~/.claude` configuration to `/srv/quanty/.claude`, with private permissions and `quanty` ownership. On platforms that store credentials outside that folder, use `setup-token` instead. Do not set `ANTHROPIC_API_KEY`: Claude calls must use the signed-in subscription. Run the sign-in commands only yourself; the installer never signs in or copies credentials. Exit the service account shell when finished.

## 4. Rust, Trunk and the build

The installer uses [rustup](https://rust-lang.org/tools/install) with the pinned Rust toolchain, adds `wasm32-unknown-unknown` and installs [Trunk](https://trunkrs.dev/). For a manual build as `quanty`:

```bash
sudo -iu quanty
export PATH="$HOME/.cargo/bin:$HOME/.npm-global/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97.1 --profile minimal
rustup target add wasm32-unknown-unknown
cargo install --locked trunk --version 0.22.0
cd /srv/quanty/src/crates/web-ui
trunk build --release
cd ../..
cargo build --release -p server
exit
sudo install -m 0755 /srv/quanty/src/target/release/quanty /usr/local/bin/quanty
```

Build the UI first: its `dist/` is embedded when the server builds. Rebuild the server after every UI build. To use stdio MCP on this host too, build `cargo build --release -p mcp` in the repository and install `target/release/quanty-mcp` in `/usr/local/bin`.

## 5. Settings and the quanty unit

Edit `/srv/quanty/.env` privately. The installer supplies database secrets, a shared login password, an MCP token and a session key. Add the Gemini and Jev keys. The example below names every server setting; [`.env.example`](../.env.example) also lists collections, caches and model defaults.

| Setting | Meaning | Default |
| --- | --- | --- |
| `QUANTY_LISTEN` | Bind address; the supplied unit explicitly overrides it with loopback | `127.0.0.1:7878` |
| `QUANTY_PASSWORD` | Shared browser login; required on a non-loopback listen address | none |
| `QUANTY_MCP_TOKEN` | Bearer token for `/mcp`; without one the route returns 404 | none |
| `QUANTY_SESSION_KEY` | 64 hex characters; keeps sessions valid across restarts | random on each start |
| `QUANTY_UPLOAD_LIMIT_MB` | Maximum PDF upload size in MiB | `1024` |
| `QDRANT_URL` | Qdrant gRPC address | `http://localhost:6334` |
| `QDRANT_API_KEY` | Qdrant API key; the deployment requires it | none |
| `FALKORDB_URL` | Redis/FalkorDB address | `falkor://localhost:6379` |
| `FALKORDB_PASSWORD` | Redis password; the deployment requires it | none |
| `FALKORDB_GRAPH` | Graph name | `quanty` |
| `CONTENT_DIR` | Converted content on this host | `content` |
| `EMBEDDING_GEMINI_API_KEY` | Embedding key | none |
| `CONVERTER_JEV_API_KEY` | Jev conversion key | none |
| `LLM_PROVIDER`, `LLM_MODEL`, `LLM_THINKING` | Model defaults, superseded by saved choices | `codex`, `gpt-6.1-sol`, `medium` |

Generate any missing secrets with `openssl rand -hex 32`. Use simple `NAME=value` or single-quoted values; the same file is read by systemd and quanty. Keep secrets out of URLs. Preserve `QUANTY_SESSION_KEY` during upgrades, or everyone must sign in again.

[quanty.service](../deploy/quanty.service) sets `User=quanty`, `WorkingDirectory=/srv/quanty`, the environment file, the service account's CLI paths, and `ExecStart=/usr/local/bin/quanty --listen 127.0.0.1:7878`. It restarts on failure. Changing the account requires changing its home and `PATH` consistently.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now quanty
sudo systemctl restart quanty
curl --fail http://127.0.0.1:7878/healthz
```

Expect `ok`. This proves the HTTP process is alive; use `rag-ingest health` or MCP `health` to check the stores and model tools.

## 6. Caddy and the first login

[Caddy's official apt instructions](https://caddyserver.com/docs/install#debian-ubuntu-raspbian) install the signing key and stable repository. The installer performs these steps:

```bash
sudo apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo chmod o+r /usr/share/keyrings/caddy-stable-archive-keyring.gpg /etc/apt/sources.list.d/caddy-stable.list
sudo apt-get update
sudo apt-get install -y caddy
```

Use [deploy/Caddyfile](../deploy/Caddyfile) with your hostname in `/etc/caddy/Caddyfile`. It proxies HTTP and WebSockets to `127.0.0.1:7878`, permits request bodies up to `1GB`, and enables zstd/gzip. Caddy manages HTTPS certificates. If the installer wrote a proposed file, review it, then install it:

```bash
sudo install -m 0644 /etc/quanty/Caddyfile.proposed /etc/caddy/Caddyfile
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
sudo systemctl reload caddy
```

Open `https://quanty.example.com` and enter `QUANTY_PASSWORD`. The HttpOnly session cookie lasts 30 days. Five wrong passwords within a minute cause a one-minute 429 response. `POST /logout` clears the cookie. Keep a password set behind Caddy: a password-free loopback server rejects the public hostname that Caddy forwards.

To connect Claude Code, use [the MCP guide](mcp.md#10-add-it-to-claude-code):

```bash
claude mcp add --transport http quanty https://quanty.example.com/mcp --header "Authorization: Bearer YOUR_QUANTY_MCP_TOKEN"
```

## 7. Back up and move a library

Finish any ingest or dataset build first. Save Redis, stop the three data-serving units, then archive the three library folders:

```bash
read -rsp 'FalkorDB password: ' REDISCLI_AUTH; echo
export REDISCLI_AUTH
redis-cli SAVE
unset REDISCLI_AUTH
sudo systemctl stop quanty qdrant redis-server
sudo tar -C /srv/quanty -czf "/var/backups/quanty-library-$(date +%F-%H%M%S).tar.gz" content data/qdrant data/falkordb
sudo systemctl start redis-server qdrant quanty
```

Back up `.env`, `.quanty-model.json` and CLI credentials separately in encrypted storage with private permissions. Keep caches and datasets too if you want resumable model work. Caddy may keep running while the data services are stopped.

To move a library, install the same store versions on the new host, stop the three units there and keep its newly created folders aside. Transfer and extract the archive under `/srv/quanty`, preserving the relative folder names. Set the new host's `.env` to its loopback stores and `/srv/quanty/content`; restore secrets and sign-ins securely. Make `content` and `data/qdrant` belong to `quanty`, and `data/falkordb` to `redis`, with traversal for Redis as the installer sets. Then start the stores and quanty, check health and the Library, and update DNS. For already ingested absolute picture paths, keep the content path identical when moving. No format migration is provided; this server release starts with a fresh library.

## 8. When a unit fails

```bash
sudo systemctl status quanty qdrant redis-server caddy
sudo journalctl -u quanty -u qdrant -u redis-server -u caddy -n 100 --no-pager
```

Check database secrets, ownership and available disk space. For Redis, confirm version 8 or newer, `libgomp1`, the module path, and the writable-path override. For a model CLI, check its `PATH` and sign-in as `quanty`, then restart quanty after editing `.env`. For a 503 missing-bundle page, build Trunk before rebuilding the server. For HTTPS, check DNS and inbound ports 80/443. See [troubleshooting](troubleshooting.md) for login, connection and upload failures.

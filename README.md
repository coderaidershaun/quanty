<h1 align="center">quanty</h1>

<p align="center">
  <b>A multimodal RAG over the PDFs of quant finance books and papers.</b><br>
  A web app for people, an MCP server for agents, and a command line, all in Rust.
</p>

<p align="center">
  <a href="#quickstart-run-it-on-your-mac">Run it on your Mac</a> ·
  <a href="#quickstart-serve-it-from-an-ubuntu-server">Serve it from Ubuntu</a> ·
  <a href="docs/guide.md">Guide</a> ·
  <a href="docs/troubleshooting.md">Troubleshooting</a> ·
  <a href="docs/mcp.md">MCP server</a> ·
  <a href="docs/reference.md">Reference</a>
</p>

<p align="center">
  <img src="docs/img/ask-screen.png" width="880" alt="The Ask screen of quanty: a question, its answer with citations, the source panel, the concept graph, the retrieval path and follow-up questions">
  <br>
  <sub>The app in the browser on its built-in <code>black-scholes</code> fixture: sample data, no store, no model, no cost. The screenshot awaits an update by the owner.</sub>
</p>

## Why quanty

On their own, LLMs cannot be trusted to automate post-trade analysis research: they hallucinate math and are not (yet) masters of quantitative trading end to end. A quant research team can instead give its agents a trusted knowledge base that it owns as its IP and moat.

quanty is an open-source answer:

- Give it PDFs of books and papers.
- Its OCR layer, run entirely by the software, uses Jev to find the math and the selected Codex or Claude model to write it as LaTeX and describe each figure.
- Everything is embedded into a multimodal RAG database and linked by concept, so the connections grow with the knowledge base.
- It is 100% Rust, the UI included.

A person can ask it "what is the correct math for a multivariate Hawkes process and how has it been used to improve volatility forecasting?", and an agent can ask the same over MCP for a post-trade analytics or trading engine. The knowledge is not magically there: the business builds it. The point is that you know the math comes from data you gave. quanty also builds training datasets for a company's own LLMs, such as Gemma 4, from the same converted documents: `rag-dataset`, the **Datasets** tab and the `build_dataset` tool. [docs/training.md](docs/training.md) says how to train on them.

Whether the goal is maximising Sharpe ratio, increasing trade volume or reducing risk, quanty is a stepping stone between the robust modelling quant finance requires and the accelerating progress of QIS.

## Quickstart: run it on your Mac

Run these commands from the root of this repository. The UI runs in your browser; the Rust server runs on your Mac.

1. **Try it first, with nothing set up.** With Rust installed, this opens the built-in sample library without keys, Docker or model calls. `--fixture list` names the other scenes.

   ```bash
   cargo install --locked trunk
   rustup target add wasm32-unknown-unknown
   cd crates/web-ui && trunk build --release && cd ../..
   cargo run --release -p server -- --fixture black-scholes --open
   ```

   Or if already built, just run with:

   ```bash
   cargo run --release -p server -- --open
   ```

   Stop the server with Ctrl-C before continuing.

2. **Install the tools for your own library.**

   | You need                                                             | What it is for                                                                         |
   | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
   | Rust with `cargo` (the build uses 1.97.1)                            | Builds the server and browser UI                                                       |
   | Docker Desktop, with `docker compose`                                | Runs Qdrant for vectors and FalkorDB for the graph                                     |
   | [Poppler](https://poppler.freedesktop.org/) (`brew install poppler`) | Reads PDFs during conversion                                                           |
   | The `codex` or `claude` CLI, signed in                               | Reads pages, finds concepts and writes answers using the selected agent's subscription |

3. **Set up the databases.** Start Docker Desktop, then run:

   ```bash
   docker compose up -d
   docker compose ps
   ```

   Both stores should be running. Quanty uses Qdrant on `6334` and FalkorDB on `6379`; the Compose file also exposes Qdrant's HTTP port `6333` and FalkorDB's browser on `3000`. Persistent store files are under `data/`.

   To start fresh, this command **deletes the local library**, including converted content:

   ```bash
   docker compose down && rm -rf data/qdrant data/falkordb content
   ```

4. **Add the keys.** Copy the example, then edit `.env` and replace `PLEASE_PROVIDE` and `ENTER` with your keys.

   ```bash
   cp .env.example .env
   unset ANTHROPIC_API_KEY
   ```

   | Key                        | What it is for                             |
   | -------------------------- | ------------------------------------------ |
   | `EMBEDDING_GEMINI_API_KEY` | Gemini embeddings for pages and questions  |
   | `CONVERTER_JEV_API_KEY`    | Jev's check for math during PDF conversion |

   > [!IMPORTANT]
   > Keep `ANTHROPIC_API_KEY` unset in both the shell and `.env`. When Claude is selected, quanty refuses to call it with that key set, so the work uses your subscription. Codex requires a ChatGPT subscription login.

5. **Check the services.** This checks Qdrant, FalkorDB and the selected Ingest agent. It does not validate Gemini or Jev keys.

   ```bash
   cargo run --release -p rag-ingestion --bin rag-ingest -- health
   ```

6. **Open your library.**

   ```bash
   cargo run --release -p server -- --open
   ```

   The browser opens on **Ask**, with the catalogue from your one library. The footer shows the connection to the server. With the example's loopback address and an empty `QUANTY_PASSWORD`, there is no login. Keep a password set when serving through a public HTTPS host.

7. **Add your first PDF.** Open **Ingest**, choose a PDF to upload, run its check, then start ingestion. Follow [Add your first PDF](docs/guide.md#add-your-first-pdf) for the choices and costs. **Share** downloads answers through your browser.

## Quickstart: serve it from an Ubuntu server

Use a dedicated **Ubuntu 24.04 LTS x86_64** host. The pinned FalkorDB module requires libraries absent from Ubuntu 22.04; other releases and architectures have not been verified. Quanty, Qdrant and Redis run on this host, and people and agents share one library. The stores stay on loopback; Caddy serves HTTPS on ports 80 and 443.

This is the manual short form of [Run quanty on an Ubuntu server, no Docker](docs/server.md), which also covers the installer, backups, moving a library and unit failures. The versions and download commands were checked against vendor sources on 22 October 2026; these instructions have not been executed on Ubuntu. Run the blocks in order in the same Bash shell over SSH, so the generated secret variables remain available. Replace `quanty.example.com` with your hostname. Version variables are at the top of the blocks that install versioned tools; Redis and Caddy use their official stable apt packages.

1. **Create the user and folders.** The service account's home is `/srv/quanty`; keep its model sign-ins there.

   ```bash
   QUANTY_USER=quanty
   QUANTY_ROOT=/srv/quanty
   sudo apt-get update
   sudo apt-get install -y ca-certificates curl gpg lsb-release openssl git nano
   sudo useradd --system --user-group --create-home --home-dir "$QUANTY_ROOT" --shell /bin/bash "$QUANTY_USER"
   sudo install -d -o "$QUANTY_USER" -g "$QUANTY_USER" -m 0750 \
     "$QUANTY_ROOT" "$QUANTY_ROOT/content" "$QUANTY_ROOT/data" \
     "$QUANTY_ROOT/data/qdrant" "$QUANTY_ROOT/data/uploads" "$QUANTY_ROOT/data/exports"
   sudo install -d -m 0755 /etc/quanty
   ```

2. **Set up the databases: Qdrant.** Download the release, generate both database secrets, and write the config and unit. Both HTTP `6333` and gRPC `6334` bind to loopback and require the API key. Quanty uses gRPC; the check below uses HTTP.

   ```bash
   QDRANT_VERSION=1.20.0
   QDRANT_API_KEY=$(openssl rand -hex 32)
   FALKORDB_PASSWORD=$(openssl rand -hex 32)
   curl -fL "https://github.com/qdrant/qdrant/releases/download/v${QDRANT_VERSION}/qdrant-x86_64-unknown-linux-gnu.tar.gz" -o /tmp/qdrant.tar.gz
   tar -xzf /tmp/qdrant.tar.gz -C /tmp
   sudo install -m 0755 /tmp/qdrant /usr/local/bin/qdrant
   sudo install -m 0600 -o quanty -g quanty /dev/null /srv/quanty/.env
   printf 'QDRANT_API_KEY=%s\nFALKORDB_PASSWORD=%s\n' "$QDRANT_API_KEY" "$FALKORDB_PASSWORD" | sudo tee /srv/quanty/.env >/dev/null
   sudo chown quanty:quanty /srv/quanty/.env
   sudo chmod 0600 /srv/quanty/.env
   sudo tee /etc/quanty/qdrant-config.yaml >/dev/null <<'EOF'
   storage:
     storage_path: /srv/quanty/data/qdrant
     snapshots_path: /srv/quanty/data/qdrant/snapshots
   service:
     host: 127.0.0.1
     http_port: 6333
     grpc_port: 6334
   telemetry_disabled: true
   EOF
   sudo tee /etc/systemd/system/qdrant.service >/dev/null <<'EOF'
   [Unit]
   Description=Qdrant for Quanty
   After=network-online.target
   Wants=network-online.target
   [Service]
   User=quanty
   Group=quanty
   WorkingDirectory=/srv/quanty
   EnvironmentFile=/srv/quanty/.env
   ExecStartPre=/usr/bin/test -n "${QDRANT_API_KEY}"
   ExecStart=/usr/bin/env QDRANT__SERVICE__API_KEY=${QDRANT_API_KEY} /usr/local/bin/qdrant --config-path /etc/quanty/qdrant-config.yaml
   Restart=on-failure
   UMask=0077
   [Install]
   WantedBy=multi-user.target
   EOF
   sudo systemctl daemon-reload
   sudo systemctl enable --now qdrant
   curl --fail -H "api-key: $QDRANT_API_KEY" http://127.0.0.1:6333/collections
   ```

   Expect JSON with the collections; a fresh library has none. Systemd maps the key from `.env` to Qdrant's actual environment setting; YAML does not expand it.

3. **Set up the databases: Redis 8 and FalkorDB.** Install Redis from its own apt repository and the checked FalkorDB module. Redis's password is rendered into its config; the writable-path override and traversal permissions let its service reach the data folder.

   ```bash
   FALKORDB_VERSION=6.0.3
   REDIS_MIN_MAJOR=8
   curl -fsSL https://packages.redis.io/gpg | sudo gpg --dearmor -o /usr/share/keyrings/redis-archive-keyring.gpg
   sudo chmod 0644 /usr/share/keyrings/redis-archive-keyring.gpg
   echo "deb [signed-by=/usr/share/keyrings/redis-archive-keyring.gpg] https://packages.redis.io/deb $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/redis.list
   sudo apt-get update
   sudo apt-get install -y redis libgomp1 acl
   redis_major=$(redis-server --version | sed -n 's/.*v=\([0-9]*\).*/\1/p')
   test "$redis_major" -ge "$REDIS_MIN_MAJOR" || { echo 'Redis 8 or newer is required.' >&2; exit 1; }
   curl -fL "https://github.com/FalkorDB/FalkorDB/releases/download/v${FALKORDB_VERSION}/falkordb-x64.so" -o /tmp/falkordb-x64.so
   curl -fL "https://github.com/FalkorDB/FalkorDB/releases/download/v${FALKORDB_VERSION}/falkordb-x64.so.sha256" -o /tmp/falkordb-x64.so.sha256
   (cd /tmp && sha256sum -c falkordb-x64.so.sha256)
   sudo install -D -m 0755 /tmp/falkordb-x64.so /opt/falkordb/falkordb.so
   sudo install -d -m 0750 -o redis -g redis /srv/quanty/data/falkordb /etc/redis/quanty
   sudo setfacl -m u:redis:--x /srv/quanty /srv/quanty/data
   sudo tee /etc/redis/quanty/falkordb.conf >/dev/null <<EOF
   loadmodule /opt/falkordb/falkordb.so
   requirepass "$FALKORDB_PASSWORD"
   bind 127.0.0.1
   protected-mode yes
   dir /srv/quanty/data/falkordb
   EOF
   sudo chown root:redis /etc/redis/quanty/falkordb.conf
   sudo chmod 0640 /etc/redis/quanty/falkordb.conf
   echo 'include /etc/redis/quanty/falkordb.conf' | sudo tee -a /etc/redis/redis.conf >/dev/null
   sudo install -d -m 0755 /etc/systemd/system/redis-server.service.d
   printf '[Service]\nReadWritePaths=/srv/quanty/data/falkordb\n' | sudo tee /etc/systemd/system/redis-server.service.d/quanty.conf >/dev/null
   sudo systemctl daemon-reload
   sudo systemctl enable redis-server
   sudo systemctl restart redis-server
   redis-cli -a "$FALKORDB_PASSWORD" PING
   redis-cli -a "$FALKORDB_PASSWORD" MODULE LIST
   ```

   Expect `PONG` and a module named `graph`.

4. **Install Poppler, Node and the two model CLIs, then sign in.** Complete Codex's device sign-in in your browser. Claude's command prints a subscription token; keep it for the `.env` step. These sign-ins belong to `quanty`.

   ```bash
   NODE_VERSION=24.22.1
   CODEX_VERSION=0.163.0
   CLAUDE_VERSION=2.1.308
   sudo apt-get install -y poppler-utils xz-utils
   curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" -o "/tmp/node-v${NODE_VERSION}-linux-x64.tar.xz"
   curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt" -o /tmp/SHASUMS256.txt
   (cd /tmp && grep " node-v${NODE_VERSION}-linux-x64.tar.xz$" SHASUMS256.txt | sha256sum -c -)
   sudo install -d -m 0755 /opt/node
   sudo tar -xJf "/tmp/node-v${NODE_VERSION}-linux-x64.tar.xz" -C /opt/node --strip-components=1
   sudo -iu quanty env PATH=/opt/node/bin:/usr/local/bin:/usr/bin:/bin \
     npm install --global --prefix /srv/quanty/.npm-global "@openai/codex@$CODEX_VERSION" "@anthropic-ai/claude-code@$CLAUDE_VERSION"
   sudo -iu quanty env PATH=/srv/quanty/.npm-global/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin codex login --device-auth
   sudo -iu quanty env PATH=/srv/quanty/.npm-global/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin codex login status
   sudo -iu quanty env PATH=/srv/quanty/.npm-global/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin claude setup-token
   ```

5. **Install Rust and Trunk, clone quanty, and build.** Build the UI before the server, which embeds the bundle. Repeat both builds whenever the UI changes.

   ```bash
   RUST_VERSION=1.97.1
   TRUNK_VERSION=0.22.0
   sudo apt-get install -y build-essential pkg-config libssl-dev
   sudo -iu quanty env RUST_VERSION="$RUST_VERSION" TRUNK_VERSION="$TRUNK_VERSION" bash <<'BUILD'
   set -eu
   export PATH="/srv/quanty/.cargo/bin:/srv/quanty/.npm-global/bin:/opt/node/bin:/usr/local/bin:/usr/bin:/bin"
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "$RUST_VERSION" --profile minimal
   rustup target add wasm32-unknown-unknown
   cargo install --locked trunk --version "$TRUNK_VERSION"
   git clone https://github.com/coderaidershaun/quanty.git /srv/quanty/src
   cd /srv/quanty/src/crates/web-ui
   trunk build --release
   cd ../..
   cargo build --release -p server
   BUILD
   sudo install -m 0755 /srv/quanty/src/target/release/quanty /usr/local/bin/quanty
   ```

6. **Write `.env` with every setting.** Keep a copy of the generated secrets in your password manager. Enter the Gemini and Jev keys, then Claude's subscription token if you will use Claude; leave that last input empty for Codex only. `ANTHROPIC_API_KEY` must stay unset. The one `.env` names the one library.

   ```bash
   QUANTY_PASSWORD=$(openssl rand -hex 32)
   QUANTY_MCP_TOKEN=$(openssl rand -hex 32)
   QUANTY_SESSION_KEY=$(openssl rand -hex 32)
   read -rsp 'Gemini API key: ' EMBEDDING_GEMINI_API_KEY; echo
   read -rsp 'Jev API key: ' CONVERTER_JEV_API_KEY; echo
   read -rsp 'Claude setup-token value (optional): ' CLAUDE_CODE_OAUTH_TOKEN; echo
   unset ANTHROPIC_API_KEY
   sudo tee /srv/quanty/.env >/dev/null <<EOF
   QUANTY_LISTEN=127.0.0.1:8080
   QUANTY_PASSWORD=$QUANTY_PASSWORD
   QUANTY_MCP_TOKEN=$QUANTY_MCP_TOKEN
   QUANTY_SESSION_KEY=$QUANTY_SESSION_KEY
   QUANTY_UPLOAD_LIMIT_MB=1024
   QDRANT_URL=http://127.0.0.1:6334
   QDRANT_API_KEY=$QDRANT_API_KEY
   FALKORDB_URL=falkor://127.0.0.1:6379
   FALKORDB_PASSWORD=$FALKORDB_PASSWORD
   FALKORDB_GRAPH=quanty
   QDRANT_ITEMS_COLLECTION=items
   QDRANT_CONCEPTS_COLLECTION=concepts
   CONTENT_DIR=/srv/quanty/content
   CONCEPT_CACHE_DIR=/srv/quanty/data/concept-cache
   CONCEPT_DECISION_LOG=/srv/quanty/data/concept-decisions.jsonl
   DATASET_DIR=/srv/quanty/datasets
   DATASET_CACHE_DIR=/srv/quanty/data/dataset-cache
   EMBEDDING_GEMINI_API_KEY=$EMBEDDING_GEMINI_API_KEY
   CONVERTER_JEV_API_KEY=$CONVERTER_JEV_API_KEY
   LLM_PROVIDER=codex
   LLM_MODEL=gpt-6.1-sol
   LLM_THINKING=medium
   CLAUDE_CODE_OAUTH_TOKEN=$CLAUDE_CODE_OAUTH_TOKEN
   EOF
   sudo chown quanty:quanty /srv/quanty/.env
   sudo chmod 0600 /srv/quanty/.env
   sudo -u quanty nano /srv/quanty/.env
   ```

   Read the generated password and MCP token privately in the editor and save them. Keep `QUANTY_SESSION_KEY` through upgrades so sessions survive restarts. Saved model choices supersede the `LLM_*` defaults. The [reference](docs/reference.md) explains each setting and server flag.

7. **Install and start the quanty unit.** The supplied unit runs as `quanty`, reads `/srv/quanty/.env`, supplies the model tools' `PATH`, and binds to `127.0.0.1:8080`.

   ```bash
   QUANTY_SOURCE=/srv/quanty/src
   sudo install -m 0644 "$QUANTY_SOURCE/deploy/quanty.service" /etc/systemd/system/quanty.service
   sudo systemctl daemon-reload
   sudo systemctl enable --now quanty
   curl --fail http://127.0.0.1:8080/healthz
   ```

   Expect `ok`. This checks the HTTP process; the app's health display checks the stores and model tools.

8. **Set DNS and install Caddy for HTTPS.** Create your hostname's DNS A record pointing at this server, and an AAAA record only if IPv6 works. Allow inbound ports 80 and 443. Use your real hostname below; keep `QUANTY_PASSWORD` set behind Caddy.

   ```bash
   QUANTY_DOMAIN=quanty.example.com
   sudo apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
   curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
   curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt | sudo tee /etc/apt/sources.list.d/caddy-stable.list
   sudo chmod o+r /usr/share/keyrings/caddy-stable-archive-keyring.gpg /etc/apt/sources.list.d/caddy-stable.list
   sudo apt-get update
   sudo apt-get install -y caddy
   sudo tee /etc/caddy/Caddyfile >/dev/null <<EOF
   $QUANTY_DOMAIN {
       request_body {
           max_size 1GB
       }
       reverse_proxy 127.0.0.1:8080
       encode zstd gzip
   }
   EOF
   sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
   sudo systemctl enable --now caddy
   sudo systemctl reload caddy
   ```

9. **Open the app and sign in.** On your Mac, open the hostname from the preceding step and enter the `QUANTY_PASSWORD` you saved. The signed HttpOnly session cookie lasts 30 days.

   ```bash
   QUANTY_DOMAIN=quanty.example.com
   open "https://$QUANTY_DOMAIN"
   ```

10. **Add the MCP endpoint to Claude Code.** In a Bash shell on the machine where you use Claude Code, enter the server's MCP token. This endpoint uses a bearer token independently of the browser login. See the [MCP guide](docs/mcp.md#10-add-it-to-claude-code) for the project config and stdio option.

    ```bash
    QUANTY_DOMAIN=quanty.example.com
    read -rsp 'Quanty MCP token: ' QUANTY_MCP_TOKEN; echo
    claude mcp add --transport http quanty "https://$QUANTY_DOMAIN/mcp" --header "Authorization: Bearer $QUANTY_MCP_TOKEN"
    unset QUANTY_MCP_TOKEN
    ```

## Documentation

- [docs/guide.md](docs/guide.md): choose a model, understand the library, add your first PDF, use the browser app and command line, costs, start fresh and artifact descriptions.
- [docs/server.md](docs/server.md): run quanty on an Ubuntu server, no Docker, with HTTPS, backups and moving a library.
- [docs/mcp.md](docs/mcp.md): the MCP server for agents, its tools and how to add it to Claude Code.
- [docs/reference.md](docs/reference.md): every command, flag and setting in full.
- [docs/training.md](docs/training.md): train Gemma 4 on a dataset that quanty built.
- [docs/troubleshooting.md](docs/troubleshooting.md): messages you may see, and what to do.
- [docs/architecture.md](docs/architecture.md): the whole system on one page, and the folders of the repository.

## Licence

MIT. See [LICENSE](LICENSE).

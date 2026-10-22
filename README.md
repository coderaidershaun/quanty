<h1 align="center">quanty</h1>

<p align="center">
  <b>A multimodal RAG over the PDFs of quant finance books and papers.</b><br>
  A desktop app for people, an MCP server for agents, and a command line, all in Rust.
</p>

<p align="center">
  <a href="#quickstart-run-the-app">Run the app</a> ·
  <a href="#quickstart-the-database-on-an-ubuntu-server">Database on a server</a> ·
  <a href="docs/guide.md">Guide</a> ·
  <a href="docs/troubleshooting.md">Troubleshooting</a> ·
  <a href="docs/mcp.md">MCP server</a> ·
  <a href="docs/reference.md">Reference</a>
</p>

<p align="center">
  <img src="docs/img/ask-screen.png" width="880" alt="The Ask screen of the quanty desktop app: a question, its answer with citations, the source panel, the concept graph, the retrieval path and follow-up questions">
  <br>
  <sub>The desktop app on its built-in <code>black-scholes</code> fixture: sample data, no store, no model, no cost.</sub>
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

## Quickstart: run the app

The shortest path to the desktop app on a laptop. Run every command in the quanty folder, the root of this repository.

**Try it first, with nothing set up.** With Rust installed and nothing else (no keys, no Docker), this opens the app shown above on its built-in sample data. `--fixture list` names the other scenes.

```bash
cargo run --release -p gui -- --fixture black-scholes
```

**Then set it up for real.**

1. **Install the tools.**

   | You need | What it is for |
   | --- | --- |
   | Rust (stable, with `cargo`) | Builds every program |
   | Docker, with `docker compose` | Runs the two stores: Qdrant (vectors) and FalkorDB (the graph) |
   | [Poppler](https://poppler.freedesktop.org/) (`brew install poppler`) | Reads a PDF when it is converted |
   | The `codex` CLI, signed in (or `claude`) | Reads pages, finds concepts and writes answers, on the subscription of each agent you choose |

2. **Start the stores.** Start Docker Desktop first, then:

   ```bash
   docker compose up -d    # Qdrant on ports 6333 and 6334, FalkorDB on 6379, data under data/
   docker compose ps       # both quanty-qdrant-1 and quanty-falkordb-1 say running
   ```

   These two stores are the database called **Local**. To use stores on a server instead, see [the server quickstart](#quickstart-the-database-on-an-ubuntu-server).

3. **Add the keys.** Copy the example and put your two keys in place of `PLEASE_PROVIDE` and `ENTER`. Then make sure `ANTHROPIC_API_KEY` is not set.

   ```bash
   cp .env.example .env
   unset ANTHROPIC_API_KEY
   ```

   | Key | What it is for |
   | --- | --- |
   | `EMBEDDING_GEMINI_API_KEY` | Gemini embeddings: it embeds every page and every question |
   | `CONVERTER_JEV_API_KEY` | The Jev API, which is asked whether a page holds math: only when a PDF is converted |

   > [!IMPORTANT]
   > `ANTHROPIC_API_KEY` must not be set, in the shell or in `.env`. When Claude is selected and it is set, quanty asks `claude` nothing, so that the work is billed to the subscription and not to the API.

4. **Check the services.** `health` prints one line each for Qdrant, FalkorDB and the agent of the Ingest choice: `ok`, or `FAILED` with what is wrong. It does not check the Gemini or Jev keys. When the Ingest choice is Claude, it refuses `ANTHROPIC_API_KEY`; Codex requires a ChatGPT subscription login.

   ```bash
   cargo run --release -p rag-ingestion --bin rag-ingest -- health
   ```

5. **Open the app.**

   ```bash
   cargo run --release -p gui
   ```

   The first time, the window opens on a sheet that asks which database to use, and nothing loads yet. Choose **Local**, which is the two Docker stores from step 2, and press **Use**. After that the window opens on **Ask**. When a store, a key or `claude` is not ready, the part of the window that needs it says which one and what to do.

To add your first PDF, follow [Add your first PDF](docs/guide.md#add-your-first-pdf) in the guide.

## Quickstart: the database on an Ubuntu server

Keep the two stores, Qdrant and FalkorDB, on an Ubuntu server, so that every machine and every agent uses one library. No quanty program runs on the server. Your machines reach the stores through an SSH tunnel, and the converted pages through a mounted folder. This is a short version of [docs/server.md](docs/server.md). Replace `me@server` with your SSH login.

**On the server**

1. **Install Docker Engine and its Compose plugin.** These are the steps of [Docker's page for Ubuntu](https://docs.docker.com/engine/install/ubuntu/), for Ubuntu 22.04, 24.04 and 26.04.

   ```bash
   sudo apt update
   sudo apt install ca-certificates curl
   sudo install -m 0755 -d /etc/apt/keyrings
   sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
   sudo chmod a+r /etc/apt/keyrings/docker.asc

   sudo tee /etc/apt/sources.list.d/docker.sources <<EOF
   Types: deb
   URIs: https://download.docker.com/linux/ubuntu
   Suites: $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}")
   Components: stable
   Architectures: $(dpkg --print-architecture)
   Signed-By: /etc/apt/keyrings/docker.asc
   EOF

   sudo apt update
   sudo apt install docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
   sudo docker run hello-world
   ```

   To run `docker` without `sudo`, add yourself to the `docker` group, which the install created, then log out and in ([Docker's page](https://docs.docker.com/engine/install/linux-postinstall/)). The steps below assume you did.

   ```bash
   sudo usermod -aG docker $USER
   ```

2. **Make the folder, the key and the password.** `/srv/quanty` is the root of the library. The second command writes the key and the password to a file that only you can read. Keep a copy in your password manager, because the app asks for both.

   ```bash
   sudo mkdir -p /srv/quanty && sudo chown "$USER" /srv/quanty
   cd /srv/quanty
   mkdir content
   printf 'QDRANT_API_KEY=%s\nFALKORDB_PASSWORD=%s\n' "$(openssl rand -hex 32)" "$(openssl rand -hex 32)" > .env
   chmod 600 .env
   ```

3. **Write the Compose file.** Both ports bind to the loopback address of the server, and Qdrant and FalkorDB each need their key or password.

   ```bash
   cat > docker-compose.yaml <<'EOF'
   services:
     qdrant:
       image: qdrant/qdrant
       ports: ["127.0.0.1:6334:6334"]
       environment:
         - QDRANT__SERVICE__API_KEY=${QDRANT_API_KEY}
       volumes: ["./data/qdrant:/qdrant/storage"]
     falkordb:
       image: falkordb/falkordb
       ports: ["127.0.0.1:6379:6379"]
       environment:
         - REDIS_ARGS=--requirepass ${FALKORDB_PASSWORD}
         - REDISCLI_AUTH=${FALKORDB_PASSWORD}
       volumes: ["./data/falkordb:/var/lib/falkordb/data"]
   EOF
   ```

4. **Start the stores and check them.**

   ```bash
   docker compose up -d
   docker compose ps                              # both say running
   docker compose exec falkordb redis-cli PING    # PONG: the password works
   ```

**On each machine that uses the library**

5. **Mount the content folder** at the same path on every machine, and before you start quanty. On a Mac, `sshfs` needs macFUSE too. On Ubuntu, install it with `sudo apt install sshfs`.

   ```bash
   sudo mkdir -p /Volumes/quanty-content && sudo chown "$USER" /Volumes/quanty-content
   sshfs me@server:/srv/quanty/content /Volumes/quanty-content -o reconnect
   ```

6. **Open the tunnel** in a terminal of its own. It prints nothing and stays open. Local ports 16334 and 16379 leave the usual 6334 and 6379 free for the Docker stores of your own machine.

   ```bash
   ssh -N -L 16334:127.0.0.1:6334 -L 16379:127.0.0.1:6379 me@server
   ```

7. **Add the database in the app.** Start quanty, click the entry at the right end of the footer, choose **New…**, fill in the seven fields, and press **Use**. The footer then reads **Database: Server · connected**.

   | Field | What to put in it |
   | --- | --- |
   | **Name** | Any name, such as `Server` |
   | **Qdrant URL** | `http://127.0.0.1:16334` |
   | **Qdrant API key** | `QDRANT_API_KEY` from `/srv/quanty/.env` on the server |
   | **FalkorDB URL** | `falkor://127.0.0.1:16379` |
   | **FalkorDB password** | `FALKORDB_PASSWORD` from `/srv/quanty/.env` on the server |
   | **Graph** | `quanty` |
   | **Content folder** | `/Volumes/quanty-content` |

[docs/server.md](docs/server.md) goes on with the other way to share the content folder (SMB), backups, moving a library to a new server, and a private network instead of a tunnel.

## Documentation

- [docs/guide.md](docs/guide.md): choose a model and a database, where a library lives, add your first PDF, the desktop app, the command line, costs, start fresh and artifact descriptions.
- [docs/server.md](docs/server.md): keep the stores on a server, step by step, with SMB, backups and moving a library.
- [docs/mcp.md](docs/mcp.md): the MCP server for agents, its tools and how to add it to Claude Code.
- [docs/reference.md](docs/reference.md): every command and flag in full.
- [docs/training.md](docs/training.md): train Gemma 4 on a dataset that quanty built.
- [docs/troubleshooting.md](docs/troubleshooting.md): messages you may see, and what to do.
- [docs/architecture.md](docs/architecture.md): the whole system on one page, and the folders of the repository.

## Licence

MIT. See [LICENSE](LICENSE).

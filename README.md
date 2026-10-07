# quanty

Turns book chapters (PDF) into a searchable knowledge base of text, formulas, figures and tables, linked across documents by the concepts they share.

## 1. Set up (once)

You need Docker, Rust, [Poppler](https://poppler.freedesktop.org/) (`brew install poppler`) and the `claude` CLI, signed in.

```bash
cp .env.example .env    # set CONVERTER_JEV_API_KEY and EMBEDDING_GEMINI_API_KEY
docker compose up -d    # Qdrant and FalkorDB
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

`health` must print three `ok` lines. Two things that trip it up:

- `ANTHROPIC_API_KEY` must not be set. The work runs on your `claude` subscription.
- Nothing else may listen on port 6379 (a local Redis hides FalkorDB).

## 2. Put a PDF through

One chapter per PDF, named `chapter-<number>-<name>.pdf`.

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --book "Option Volatility and Pricing" \
  --author "Sheldon Natenberg" --tag options \
  samples/chapter-1-sample-pages.pdf
```

This converts every page, stores the items, and links their concepts. `--author` and `--tag` are optional. A 20-page chapter takes about 15 minutes.

- If it stops, run the same command again. It carries on where it stopped.
- Running it again on a finished PDF does nothing and costs nothing.

## 3. Ask a question

```bash
cargo run --release -p rag-retrieval --bin rag-query -- "What is the Black–Scholes formula for a call option?"
```

| Add | To get |
| --- | --- |
| `--answer` | A written answer with its sources, in place of the list of results |
| `--kind formula` | Only formulas (or `chunk`, `figure`, `table`) |
| `--tag options` | Only documents with that tag (also `--book`, `--author`) |

## 4. Open the desktop app

```bash
cargo run --release -p gui
```

Run it in the quanty folder. The program it builds is `quanty` (`target/release/quanty`).

One window: ask a question, read the answer with its citations, and see the page each citation stands on, the concepts that link the results, and the steps the search took. It uses what steps 1 to 3 set up: Qdrant and FalkorDB running, `EMBEDDING_GEMINI_API_KEY` in `.env`, `claude` signed in, and `ANTHROPIC_API_KEY` not set.

- An ask costs one embedding call and one Sonnet call. The mode **Results only** costs the embedding call alone.
- A citation opens its page when the chapter's folder is under `content/`, which is where `rag-ingest pdf` puts it.
- To look at the app with no store, no model and no cost, run `cargo run --release -p gui -- --fixture black-scholes`. `--fixture list` names the other scenes.
- Started from another folder, the program looks for the quanty folder by its `.env`. `--home <folder>` names it outright.

| Key | What it does |
| --- | --- |
| `/` or `⌘K` | Write a question (`Enter` asks it) |
| `J`, `K` | Next and previous result |
| `⌘.` or `Esc` | Stop the search or the answer |
| `⇧⌘S` | Copy the answer with its citations |
| `⌘]`, `⌘[` | Next and previous page of the source |
| `⌘+`, `⌘−`, `⌘0` | Zoom the page while the pointer is over it |

Not built yet: the Library and Ingest pages, notices, help and the health check. Until then documents are added and labelled with `rag-ingest`.

## 5. Let an AI agent use it

`quanty-mcp` is an MCP server: an agent can search the books, read the pages, get answers with their sources, and send a chapter PDF to be ingested.

```bash
cargo build --release -p mcp    # makes target/release/quanty-mcp
```

How to start it, its seven tools and what each one costs are in [docs/mcp.md](docs/mcp.md).

## Other commands

| Command | What it does |
| --- | --- |
| `rag-ingest <picture.png> --note "<what it is>"` | Ingests a chart on its own |
| `rag-ingest tag <document id> --add <tag> --remove <tag>` | Changes the tags of a stored document |
| `rag-ingest delete-document <document id>` | Removes a document from both stores |

Run each as `cargo run --release -p rag-ingestion --bin rag-ingest -- …` or `cargo run --release -p rag-retrieval --bin rag-query -- …`. The document id is printed by every ingest.

Everything else is in [docs/reference.md](docs/reference.md).

# quanty

quanty turns book chapters (PDF) into a searchable knowledge base of text, formulas, figures and tables, linked across documents by the concepts they share. You ask it a question in a desktop app, from an AI agent over MCP, or on the command line, and the answer comes with the pages it stands on.

![The Ask screen of the quanty desktop app: a question, its answer with citations, the source page, the concept graph, the retrieval path and follow-up questions](docs/img/ask-screen.png)

*The desktop app on its built-in `black-scholes` fixture: sample data, no store, no model, no cost.*

| I want to | Go to |
| --- | --- |
| Use the desktop app | [Use the desktop app](#use-the-desktop-app) |
| Connect an AI agent over MCP, or I am that agent | [Use it from an agent (MCP)](#use-it-from-an-agent-mcp) |
| Work on the command line | [Command line](#command-line) |

All three read the same stores, so do [Set up once](#set-up-once) first. To look at the app before any set up (it needs no keys and no Docker):

```bash
cargo run --release -p gui -- --fixture black-scholes
```

## Set up once

Run every command in the quanty folder, the root of this repository. You need:

| What | What it is for |
| --- | --- |
| Rust (stable, with `cargo`) | Builds every program |
| Docker, with `docker compose` | Runs the two stores: Qdrant (vectors) and FalkorDB (the graph) |
| [Poppler](https://poppler.freedesktop.org/) (`brew install poppler`) | Reads a PDF when a chapter is converted |
| The `claude` CLI, signed in | Reads pages, finds concepts and writes answers, on your `claude` subscription |
| `EMBEDDING_GEMINI_API_KEY` | Gemini embeddings: every ingest and every question |
| `CONVERTER_JEV_API_KEY` | The Jev API, which says whether a page holds maths: only when a PDF is converted |

`ANTHROPIC_API_KEY` must **not** be set, in the shell or in `.env`. While it is set, quanty asks `claude` nothing, so that the work is billed to the subscription and not to the API.

### 1. Fill in the keys

```bash
cp .env.example .env
unset ANTHROPIC_API_KEY    # does nothing when it is not set
```

Open `.env` and put your keys in place of the two placeholders, `CONVERTER_JEV_API_KEY=PLEASE_PROVIDE` and `EMBEDDING_GEMINI_API_KEY=ENTER`. Every other line is optional and commented out, with its default. A value in the environment wins over `.env`.

### 2. Start the stores

```bash
docker compose up -d
```

Qdrant listens on ports 6333 and 6334, FalkorDB on 6379. Both keep their data under `data/`.

### 3. Check

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- health
# Qdrant: ok (http://localhost:6334)
# FalkorDB: ok (falkor://localhost:6379)
# claude: ok (signed in)
```

Three `ok` lines mean the stores and `claude` are ready. A line that says `FAILED` names what is wrong, and the command ends with exit code 1. It does not check the two keys or `ANTHROPIC_API_KEY`.

### 4. Put a first chapter in

One chapter per PDF, named `chapter-<number>-<name>.pdf`. The repository holds a sample of seven pages:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --book "Option Volatility and Pricing" \
  --author "Sheldon Natenberg" --tag options \
  samples/chapter-1-sample-pages.pdf
```

This converts every page, stores the items and links their concepts. It is paid work: see [Costs](#costs). `--author` and `--tag` are optional. A 20-page chapter takes about 15 minutes. If it stops, run the same command again: it carries on where it stopped. On a finished PDF the command does nothing and costs nothing.

### 5. Ask a question

```bash
cargo run --release -p rag-retrieval --bin rag-query -- "What is the Black–Scholes formula for a call option?"
```

It prints the stored items it found. Now the app and an agent have something to search.

## Use the desktop app

```bash
cargo run --release -p gui
```

Run it in the quanty folder. The program it builds is `target/release/quanty`. Started from another folder, it finds the quanty folder by its `.env`, and `--home <folder>` or `QUANTY_HOME` names it outright. When a store, a key or `claude` is not ready, the part of the window that needed it says which one and what to do.

To look at the app with no store, no model and no cost, run it on built-in data:

```bash
cargo run --release -p gui -- --fixture black-scholes   # a question with its full answer, as in the picture
cargo run --release -p gui -- --fixture list            # names the 19 scenes, such as stores-down and ingest-ready
```

What an ask costs depends on the mode beside the question. In the mode **Answer** it costs one Gemini embedding call and one Sonnet call on your `claude` subscription. In the mode **Results only** no answer is written, and it costs the embedding call alone.

The window has two tabs, **Ask** and **Ingest**. Ask is one screen of six panels:

| Panel | What it shows |
| --- | --- |
| Ask bar | The question, the mode, and the book, author and tag filters |
| Answer | The written answer with its citations, and the results by kind: all, formulas, figures, tables |
| Source in Context | The page a citation stands on, with its figures, formulas, tables and concepts |
| Concept Graph | The concepts of the answer and how they link |
| Retrieval Path | The five steps of the search, with what each one produced |
| Follow up | Questions to ask next and a box for your own: each is a new search with the same mode and filters |

| Key | What it does |
| --- | --- |
| `⌘1`, `⌘3` | Go to Ask, go to Ingest |
| `/` or `⌘K` | Write a question (`Enter` asks it) |
| `J`, `K` | Next and previous result |
| `⌘.` or `Esc` | Stop the search or the answer |
| `⇧⌘S` | Copy the answer with its citations |
| `⌘]`, `⌘[` | Next and previous page of the source |
| `⌘+`, `⌘−`, `⌘0` | Zoom the page while the pointer is over it |

- `/`, `J`, `K` and `Esc` do this only while no text box has the keyboard.
- A citation opens its page when the chapter's folder is found: the folder it was ingested from, or the same chapter under `content/`, which is where `rag-ingest pdf` puts it.
- Add a chapter from the app on the **Ingest** tab (`⌘3`): choose the PDF, choose the book or add a new one, check it, then start. The file must be named `chapter-<number>-<name>.pdf`. The check is free; a start is paid work, the same as `rag-ingest pdf` in [step 4](#4-put-a-first-chapter-in): see [Costs](#costs). Keep the app open while it runs; if it stops, start the same PDF again and it carries on.
- Not built yet: the Library page, the notices tray, the help sheet and the health check. Documents are labelled again and deleted with `rag-ingest`: see [Command line](#command-line).

## Use it from an agent (MCP)

`quanty-mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server with seven tools: an agent can search the books, read their pages, get an answer with its sources, and send a chapter PDF to be ingested. It uses what [Set up once](#set-up-once) made ready. The full detail is in [docs/mcp.md](docs/mcp.md).

### 1. Build it and add it to Claude Code

Run both lines in the quanty folder:

```bash
cargo build --release -p mcp    # makes target/release/quanty-mcp
claude mcp add --scope user quanty -- sh -c "cd '$PWD' && exec '$PWD/target/release/quanty-mcp'"
```

`--scope user` adds the server for every project, and `$PWD` writes the absolute path of the quanty folder into the entry. The server must start in the quanty folder, because it finds `.env`, `content/` and `data/` from the folder it starts in: that is what the `cd` is for.

To add it for one project only, leave out the second line and put this `.mcp.json` in that project's folder, with `/path/to/quanty` replaced by the absolute path of the quanty folder:

```json
{
  "mcpServers": {
    "quanty": {
      "command": "sh",
      "args": ["-c", "cd '/path/to/quanty' && exec '/path/to/quanty/target/release/quanty-mcp'"]
    }
  }
}
```

Both start the server over standard input and output. To serve over HTTP, start it yourself in the quanty folder and add its address:

```bash
target/release/quanty-mcp --http 8321
claude mcp add --transport http quanty http://127.0.0.1:8321/mcp
```

The HTTP server listens on `127.0.0.1` only and has no login: any program on this machine can call it. In `.mcp.json` its entry is `"quanty": { "type": "http", "url": "http://127.0.0.1:8321/mcp" }`. Another agent that speaks MCP takes the same command or the same address.

### 2. The tools

| Tool | What it does | Arguments | Cost |
| --- | --- | --- | --- |
| `health` | Says whether Qdrant, FalkorDB and the `claude` sign-in are ready | None | FREE |
| `list_documents` | Lists every stored document with its book, author, tags and chapter | None | FREE |
| `search` | Finds the stored items (text chunks, formulas, figures, tables) nearest to a question | `question`; optional `kind`, `book`, `author`, `tags`, `limit`, `explain` | PAID, small: one Gemini embedding call |
| `read_page` | Reads the pieces of one page of a chapter, in reading order | `document_id`, `page` | FREE, and it needs no store |
| `answer` | Writes an answer from what `search` finds, with its sources | `question`; optional `kind`, `book`, `author`, `tags` | PAID: a Gemini call and your `claude` subscription usage; a minute or two |
| `ingest_pdf` | Converts one chapter PDF and stores it | `book`; `path`, or `pdf_base64` with `file_name`; optional `author`, `tags` | PAID and slow: `claude`, Jev and Gemini; minutes for a chapter |
| `ingest_status` | Says how an ingest job is going | `job_id` | FREE |

An agent that can write its own answer should call `search`, not `answer`. A failure comes back as a tool error whose text says what to do, and it never ends the connection.

### 3. First calls to try

1. `health` with no arguments: `healthy` must be `true`.
2. `list_documents` with no arguments: the documents that are stored, each with its `document_id`.
3. `search` with `{"question": "What is the Black–Scholes formula for a call option?"}`: then give the `document_id` and `page` of a result to `read_page`.

### 4. Send a PDF

1. Name the file `chapter-<number>-<name>.pdf`, such as `chapter-1-financial-contracts.pdf`. A file with another name must be copied or renamed first. It must be a PDF of at most 50 MiB.
2. Call `ingest_pdf` with `book` (the title of the book) and `path`, the absolute path of the file on the machine the server runs on. A client that cannot reach that disk sends `pdf_base64` (standard base64) with `file_name` instead. Give exactly one of the two.
3. The call answers once the paid work has begun, with a `job_id` and `state: "running"`.
4. Call `ingest_status` with that `job_id` every 20 to 30 seconds until `state` is not `running`. It ends as `done` (with `document_id`, `items` and `summary`), as `already_ingested` (both stores held this PDF, so it cost nothing) or as `failed` (`error` says why). After `failed`, send the same PDF again to go on: converted pages and kept answers are not paid for twice.

One ingest runs at a time in a server. Over standard input and output a running job stops when the client closes the server, and a restarted server forgets its job ids: send the PDF again.

## Command line

| Program | What it does | Run it as |
| --- | --- | --- |
| `rag-ingest` | Checks the services, ingests, labels and deletes documents | `cargo run --release -p rag-ingestion --bin rag-ingest -- …` |
| `rag-query` | Asks the stored items a question | `cargo run --release -p rag-retrieval --bin rag-query -- …` |
| `ocr` | Converts a chapter PDF into saved files under `content/`, and stores nothing | `cargo run --release -p ocr -- …` |

One example of each:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf --book "Option Volatility and Pricing" samples/chapter-1-sample-pages.pdf
cargo run --release -p rag-retrieval --bin rag-query -- --answer "What is the Black–Scholes formula for a call option?"
cargo run --release -p ocr -- --book "Option Volatility and Pricing" samples/chapter-1-sample-pages.pdf
```

| More commands | What it does |
| --- | --- |
| `rag-ingest <chapter folder>` | Ingests a chapter that is already converted, and always embeds it again |
| `rag-ingest <picture.png> --note "<what it is>"` | Ingests a chart or another picture on its own |
| `rag-ingest tag <document id> --add <tag> --remove <tag>` | Changes the tags of a stored document (also `--author`) |
| `rag-ingest delete-document <document id>` | Removes a document from both stores |
| `rag-query --kind formula --tag options "<question>"` | Only formulas (or `chunk`, `figure`, `table`), and only documents with that tag (also `--book`, `--author`) |

Every ingest prints the `document id`. Each command is described in full in [docs/reference.md](docs/reference.md).

## Costs

| What | Billed to | When |
| --- | --- | --- |
| Gemini embeddings | Your Gemini API key, by the token | Every ingest, and every question from the app, `rag-query`, `search` or `answer` |
| Jev | Your Jev API key | The pages of a PDF, when they are converted |
| `claude` (Haiku and Sonnet) | The subscription `claude` is signed in to, never the API | Converting pages, finding concepts and writing an answer |

- Converted pages and the answers about concepts are kept on disk and are not paid for twice, so a run that stopped goes on from there. A PDF that is already ingested costs nothing.
- `rag-ingest pdf` checks the Gemini key and both stores before the first page is converted, so a missing key or a store that is down stops it before any page is paid for.
- These cost nothing: Qdrant and FalkorDB, which run on your machine, and `health`, `--fixture`, `list_documents`, `read_page`, `ingest_status`, `tag` and `delete-document`.

## Troubleshooting

| What you see | What to do |
| --- | --- |
| `could not reach Qdrant at …` or `could not connect to FalkorDB at …` | Run `docker compose up -d`, then `health` again |
| `did not answer like FalkorDB; another program may be using that port` | Something else listens on port 6379, such as a local Redis. Stop it, or map the container to another port and set `FALKORDB_URL` to match |
| `EMBEDDING_GEMINI_API_KEY is not set` or `CONVERTER_JEV_API_KEY is not set` | Put the key in `.env`, and start the command in the quanty folder: `.env` is looked for in the current folder and the folders above it |
| `ANTHROPIC_API_KEY is set, so claude could bill the API` or `claude is not signed in` | Run `unset ANTHROPIC_API_KEY`, or run `claude` in a terminal and sign in. Then start the program again |
| "The page was not found" in the app, or `no converted chapter under content has the document id …` from `read_page` | The chapter's folder is not where quanty looks. Put it inside a book folder under `content/`, or ingest its PDF again with `rag-ingest pdf` |

## Project layout

Gemini embeds text, formulas and pictures into one vector space, which Qdrant holds. Claude finds the concepts that each item mentions, and FalkorDB keeps them as a graph. A search takes the nearest items and follows shared concepts to related ones in other documents. More is in [docs/reference.md](docs/reference.md#how-it-works).

| Crate | What it is |
| --- | --- |
| `crates/ocr` | Turns a chapter PDF into saved pieces: headings, text, formulas, figures, tables and footnotes (`ocr`) |
| `crates/rag-core` | What the others share: the settings, the ids, the Gemini embedder, the Qdrant stores and the `claude` CLI |
| `crates/graph` | The graph store, on FalkorDB |
| `crates/rag-ingestion` | Chunks, embeds and stores a chapter, and finds and links its concepts (`rag-ingest`) |
| `crates/rag-retrieval` | Vector search, graph expansion, ranking, and answers with citations (`rag-query`) |
| `crates/gui` | The desktop app (`quanty`) |
| `crates/mcp` | The MCP server (`quanty-mcp`) |

`samples/` holds the sample PDF and converted sample chapters, and `docs/` the reference. `content/` (converted chapters) and `data/` (the stores, the kept answers) are made on your machine and are not in git.

## Licence

MIT. See [LICENSE](LICENSE).

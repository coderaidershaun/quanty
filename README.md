# quanty

## Initiative

On their own, LLMs cannot be trusted to automate post-trade analysis research: they hallucinate math and are not (yet) masters of quantitative trading end to end. A quant research team can instead give its agents a trusted knowledge base that it owns as its IP and moat.

quanty is an open-source answer. Give it PDFs of books and papers. Its OCR layer, run entirely by the software, uses Jev to find the math and Claude models to write it as LaTeX and describe each figure. Everything is embedded into a multimodal RAG database and linked by concept, so the connections grow with the knowledge base. It is 100% Rust, the UI included.

A person can ask it "what is the correct math for a multivariate Hawkes process and how has it been used to improve volatility forecasting?", and an agent can ask the same over MCP for a post-trade analytics or trading engine. The knowledge is not magically there: the business builds it. The point is that you know the math comes from data you gave. A next step is training datasets for a company's own LLMs, such as Gemma 4.

Whether the goal is maximising Sharpe ratio, increasing trade volume or reducing risk, quanty is a stepping stone between the robust modelling quant finance requires and the accelerating progress of QIS.

## See the app

![The Ask screen of the quanty desktop app: a question, its answer with citations, the source page, the concept graph, the retrieval path and follow-up questions](docs/img/ask-screen.png)

*The desktop app on its built-in `black-scholes` fixture: sample data, no store, no model, no cost.*

With Rust installed and nothing else (no keys, no Docker), run this in the quanty folder, the root of this repository:

```bash
cargo run --release -p gui -- --fixture black-scholes
```

`--fixture list` names the other scenes. People use the [desktop app](#run-the-desktop-app), agents use the [MCP server](#mcp), and there is a [command line](#command-line). All three read the same stores, so [set up once](#set-up-once) first.

## Set up once

| You need | What it is for |
| --- | --- |
| Rust (stable, with `cargo`) | Builds every program |
| Docker, with `docker compose` | Runs the two stores: Qdrant (vectors) and FalkorDB (the graph) |
| [Poppler](https://poppler.freedesktop.org/) (`brew install poppler`) | Reads a PDF when it is converted |
| The `claude` CLI, signed in | Reads pages, finds concepts and writes answers, on your `claude` subscription |
| `EMBEDDING_GEMINI_API_KEY` | Gemini embeddings: every ingest and every question |
| `CONVERTER_JEV_API_KEY` | The Jev API, which says whether a page holds math: only when a PDF is converted |

`ANTHROPIC_API_KEY` must **not** be set, in the shell or in `.env`. While it is set, quanty asks `claude` nothing, so that the work is billed to the subscription and not to the API.

Run every command in the quanty folder:

```bash
cp .env.example .env       # then put your two keys in place of PLEASE_PROVIDE and ENTER
unset ANTHROPIC_API_KEY    # does nothing when it is not set
docker compose up -d       # Qdrant on ports 6333 and 6334, FalkorDB on 6379, data under data/
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

`health` prints one line each for Qdrant, FalkorDB and `claude`: `ok`, or `FAILED` with what is wrong. It does not check the two keys or `ANTHROPIC_API_KEY`.

Then put a first PDF in and ask a question. The library holds media. A media is a book, a paper or another work, with a title, a category, its authors and its tags. Each PDF is one document of a media: a chapter of a book, or the PDF of a paper or another work, which has a title. A document can also have tags of its own. A book's chapter is named `chapter-<number>-<name>.pdf`. The repository holds a sample of seven pages:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --book "Option Volatility and Pricing" \
  --author "Sheldon Natenberg" --tag options \
  samples/chapter-1-sample-pages.pdf

cargo run --release -p rag-retrieval --bin rag-query -- "What is the Black–Scholes formula for a call option?"
```

The PDF of a paper can have any name, and a paper often has more than one author:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --paper "Hawkes Processes in Finance" \
  --author "First Author" --author "Second Author" --tag hawkes \
  --doc-tag survey \
  path/to/hawkes-notes.pdf
```

`--author`, given once for each author, and `--tag` label the media. They are used only when the library does not have that media yet: change a stored media with `rag-ingest media`. `--doc-tag` tags this PDF only. All three are optional. A media that the library already has keeps its own category, whatever `--book`, `--paper` or `--other` says.

The ingest is paid work: see [Costs](#costs). While it runs, it prints its progress on standard error: `checking what is already stored`, `opening the pdf`, `converting 7 of 7 pages (0 already done)`, one line for each page as it is cut out, such as `page 3 cut out`, one line for each page with what the page cost, such as `page 3 converted ($0.14)`, then `writing the graph`, `embedding <n> items`, `storing the items`, `reading the concepts of <n> items` and `linking the concepts of <n> items`. The summary comes last, on standard output. A 20-page PDF takes about 15 minutes. If it stops, run the same command again and it carries on. On a finished PDF it does nothing and costs nothing.

## Run the desktop app

```bash
cargo run --release -p gui    # builds and opens target/release/quanty
```

The window has three tabs:

- **Ask**: the question with its mode and four filters, **Media**, **Authors**, **Tags** and **Category**, then the answer with its citations, the page each citation stands on, the concept graph, the steps of the search, and questions to ask next. The mode **Answer** costs one Gemini embedding call and one Sonnet call on your `claude` subscription. **Results only** writes no answer and costs the embedding call alone.
- **Library**: a card for each media, with its title, a badge for its category, its authors and its tags. Its pencil, **✎ Edit**, opens a form for the category, the authors and the tags. **Save media** writes them to the media and to every document of it, as `rag-ingest media` does, and **Cancel** closes the form. The title cannot change, because it names the media's folder. In the card, each document shows its chapter or its title, its pages, its items and its own tags. **Copy id** copies the document id that `rag-ingest tag` and `rag-ingest delete-document` take, **Read** opens the document in Ask, **Edit tags** changes its own tags, as `rag-ingest tag` does, and **Delete** deletes the document. **Delete** on a media card, beside **✎ Edit**, deletes the media with every document of it; the group "No media" has none, so its documents are deleted one by one. A sheet asks first: it names the document or the media, says what goes and that it cannot be undone, and offers **Delete document** or **Delete media**, and **Cancel** (`Esc` or a click beside the sheet is **Cancel**). A delete does what `rag-ingest delete-document` and `rag-ingest delete-media` do. It removes the passages, formulas, figures and tables from the library, the converted pages from the disk, and the copy of the PDF that the MCP server saved under `content/_uploads`; for a media, that is every document of it and the media. It never removes the PDF you picked from your own disk, the concepts, or any other media or document, and a media whose last document is deleted stays in the list with no document. A delete that fails says why on its card, the library is read again, and **Delete** again finishes it. A check of a PDF on the Ingest tab is made again after a delete. Every edit and every delete is free, and none can be made while an ingest runs or while another change is on its way: a **Delete** that is off says why when the pointer rests on it.
- **Ingest**: the **Add media** journey. Choose a media in the **Media** list, or choose **Add new media…**, pick its category, type its title, authors and tags, and press **Save media**: a saved media stays in the list, also before its first PDF. The media then shows as a card whose labels are fixed, with its pencil to edit them. Press **Choose a PDF**. The PDF of a book takes a chapter number and a chapter name, filled in from a file named `chapter-<number>-<name>.pdf`. The PDF of a paper or another media takes a title, filled in with the media's title. **Tags for this PDF** are the document's own tags. The check then runs by itself, with a bar, and is free: it says how many pages the PDF has and how many are converted already, or what would stop a start. **Start ingest** is paid work, the same as `rag-ingest pdf` above. The bar then shows the stage and how far it has got, from "Checking what is already stored" and "Opening the PDF" to "Preparing the pages — 7 of 12 ready · about 9%" and "Converting the pages — 3 of 12 done · about 36%", with what the pages have cost so far, and then the later stages up to "Linking the concepts". The percent is an estimate: it runs from the first page cut out to the last page saved. Keep the app open while it runs; if it stops, start the same PDF again and it carries on. **Add another PDF** keeps the media chosen for the next PDF.

| Key | What it does |
| --- | --- |
| `⌘1`, `⌘2`, `⌘3` | Go to Ask, Library, Ingest |
| `/` or `⌘K` | Write a question (`Enter` asks it) |
| `J`, `K` | Next and previous result |
| `⌘.` or `Esc` | Stop the search or the answer |
| `Esc` | Put a maximised panel back, when no search or answer is running |
| `⇧⌘S` | Copy the answer with its citations |

In Ask, **Maximise**, at the top right of the page's panel and of the concept graph, gives that panel the whole tab, and **Restore**, in the same place, puts every panel back. The six panels of Ask and every key are in [docs/reference.md](docs/reference.md#the-desktop-app). When a store, a key or `claude` is not ready, the part of the window that needed it says which one and what to do.

Not built yet: the notices tray, the help sheet and the health check.

## MCP

`quanty-mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server over the same stores. Build it and add it to Claude Code, with both lines run in the quanty folder. The server finds `.env`, `content/` and `data/` from the folder it starts in: that is what the `cd` is for.

```bash
cargo build --release -p mcp    # makes target/release/quanty-mcp
claude mcp add --scope user quanty -- sh -c "cd '$PWD' && exec '$PWD/target/release/quanty-mcp'"
```

| Tool | What it does | Cost |
| --- | --- | --- |
| `health` | Says whether Qdrant, FalkorDB and the `claude` sign-in are ready | Free |
| `list_documents` | Lists every stored document | Free |
| `search` | Finds the stored items nearest to a question | One small Gemini call |
| `read_page` | Reads one page of a document, in reading order | Free |
| `answer` | Writes an answer from what `search` finds, with its sources | A Gemini call and your `claude` subscription usage |
| `ingest_pdf` | Converts one PDF of a media and stores it | Paid and slow: `claude`, Jev and Gemini |
| `ingest_status` | Says how an ingest job is going | Free |

An agent that can write its own answer should call `search`, not `answer`. The arguments, the first calls to try, `.mcp.json`, HTTP and how to send a PDF are in [docs/mcp.md](docs/mcp.md).

## Command line

| Program | What it does | Run it as |
| --- | --- | --- |
| `rag-ingest` | Checks the services, ingests a PDF or a converted document, changes a media (`media`) or the own tags of a document (`tag`), and deletes a document (`delete-document`) or a whole media (`delete-media`) | `cargo run --release -p rag-ingestion --bin rag-ingest -- …` |
| `rag-query` | Asks the stored items a question, and `--answer` writes an answer | `cargo run --release -p rag-retrieval --bin rag-query -- …` |
| `ocr` | Converts the PDF of a book's chapter into saved files under `content/`, and stores nothing | `cargo run --release -p ocr -- …` |

Every command and flag is in [docs/reference.md](docs/reference.md).

## Costs

| What | Billed to | When |
| --- | --- | --- |
| Gemini embeddings | Your Gemini API key, by the token | Every ingest and every question |
| Jev | Your Jev API key | The pages of a PDF, when they are converted |
| `claude` (Haiku and Sonnet) | The subscription `claude` is signed in to, never the API | Converting pages, finding concepts and writing an answer |

Every ingest and every question reports the tokens each model used and about what they cost, such as `cost of this run: ≈ $1.75`. The figure comes from one table of prices per million tokens: it is what the API would charge, not what your subscription bills. The table, its date and how each figure is made are in [docs/reference.md](docs/reference.md#costs).

Converted pages and the answers about concepts are kept on disk, so a run that stopped goes on without paying twice, and a PDF that is already ingested costs nothing. `rag-ingest pdf` checks the Gemini key and both stores before the first page is paid for. These cost nothing: the two stores, which run on your machine; `health`, `--fixture`, `list_documents`, `read_page`, `ingest_status`, `media`, `tag`, `delete-document` and `delete-media`; and the check, every edit and every delete of the desktop app. `delete-document`, `delete-media` and a delete in the desktop app cost nothing to run, but they remove the converted pages, so the same PDF ingested again is converted again, which is paid work.

## Start fresh

To empty the library and begin again, stop the stores and remove the two folders that quanty made on your machine:

```bash
docker compose down       # stops Qdrant and FalkorDB
rm -rf data content       # the stores, the kept answers and the converted documents
docker compose up -d
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

This deletes every stored media and document, every converted page (with the PDFs that the MCP server saved under `content/_uploads`), every kept answer about concepts and the log of concept decisions. It cannot be undone. Nothing in git is touched: `samples/` stays. To ingest the PDFs again is paid work again, because nothing is kept to go on from: see [Costs](#costs). When `.env` names other places with `CONTENT_DIR`, `CONCEPT_CACHE_DIR` or `CONCEPT_DECISION_LOG`, remove those too.

## Troubleshooting

| What you see | What to do |
| --- | --- |
| `could not reach Qdrant at …` or `could not connect to FalkorDB at …` | Run `docker compose up -d`, then `health` again |
| `did not answer like FalkorDB; another program may be using that port` | Something else listens on port 6379, such as a local Redis. Stop it, or map the container to another port and set `FALKORDB_URL` to match |
| `EMBEDDING_GEMINI_API_KEY is not set` or `CONVERTER_JEV_API_KEY is not set` | Put the key in `.env`, and start the command in the quanty folder: `.env` is looked for in the current folder and the folders above it |
| `ANTHROPIC_API_KEY is set, so claude could bill the API` or `claude is not signed in` | Run `unset ANTHROPIC_API_KEY`, or run `claude` in a terminal and sign in. Then start the program again |
| "The page was not found" in the app, or `no converted chapter under content has the document id …` from `read_page` | The document's folder is not where quanty looks. Put it inside its media's folder under `content/`, or ingest its PDF again with `rag-ingest pdf` |
| `… cannot be ingested as a book chapter; rename it to chapter-<number>-<name>.pdf …` or `… so its pdf must be named chapter-<number>-<name>.pdf …` from `rag-ingest pdf` | The media is a book, so the file name gives the chapter. Rename the file. A media that the library does not have yet can be a paper or another media instead: give `--paper` or `--other`, whose PDF can have any name |

## Layout

Seven crates under `crates/`: `ocr`, `rag-core`, `graph`, `rag-ingestion`, `rag-retrieval`, `gui` and `mcp`. What each one is, and how a search works, is in [docs/reference.md](docs/reference.md#how-it-works). `samples/` holds the sample PDF and converted sample chapters. `content/` (the converted documents, in one folder for each media) and `data/` (the stores, the kept answers) are made on your machine and are not in git.

## Licence

MIT. See [LICENSE](LICENSE).

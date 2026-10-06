# quanty

A multimodal RAG layer with a knowledge graph, built on top of `ocr`.

PDFs are turned into searchable items (text, LaTeX formulas and figures). An LLM extracts the concepts each item mentions. Concepts are global, so two PDFs that both discuss Black–Scholes attach to the same node, and every new PDF links itself to everything already ingested with no manual work.

## How it works

- **Qdrant** stores every vector (items and concepts).
- **FalkorDB** stores the graph: documents, items and concepts, joined by `HAS_ITEM`, `NEXT`, `MENTIONS` and `RELATES_TO` edges.
- **Gemini Embedding 2** embeds text, LaTeX and images into one vector space.
- **Claude** (via `claude -p`) extracts and resolves concepts.
- Retrieval finds the closest items by vector search, then follows concepts across documents to pull in related material.

## Crates

| Crate | Purpose |
| --- | --- |
| `ocr` | PDF to text, LaTeX, figures and figure descriptions |
| `rag-core` | Shared types, IDs, config, `Embedder` and `Llm` traits, Qdrant wrapper |
| `graph` | `GraphStore` trait and FalkorDB implementation |
| `rag-ingestion` | Chunking, embedding, concept extraction and resolution (`rag-ingest`) |
| `rag-retrieval` | Vector search, graph expansion, ranking (`rag-query`) |

## Getting started

```bash
cp .env.example .env   # add your API keys
docker compose up -d   # Qdrant + FalkorDB
```

The `claude` CLI must be on your `PATH` and signed in.

## Converting a chapter

```bash
cargo run -p ocr -- --book "Option Volatility and Pricing" chapter-1-sample-pages.pdf
```

The file must be named `chapter-<number>-<name>.pdf`. Every page is broken into its pieces (headings, text, formulas, figures, tables and footnotes) and saved under `content/<book-title>/chapter-<number>/page-num-<i>/`, with a `page.json` for each page and a `chapter.json` for the chapter. Each figure also gets its own picture, `NN-figure.png`, cut out of the page beside its `.md` file, and a figure that cannot be cut out keeps the whole page as its picture and is listed in the summary. `--out <folder>` saves somewhere other than `content`. The command only saves files: nothing is put into Qdrant or FalkorDB.

The command reads `CONVERTER_JEV_API_KEY` from `.env`, and it refuses to run while `ANTHROPIC_API_KEY` is set, so the work is billed to the `claude` subscription. A second run on a finished chapter makes no outside call and costs nothing. An interrupted run picks up at the pages that are missing. A different PDF for the same chapter is refused.

Later crates read a chapter back, in reading order, with `ocr::read_chapter`.

## Checking the stores

```bash
cargo run -p rag-ingestion --bin rag-ingest -- health
```

Prints one line each for Qdrant, FalkorDB and the `claude` sign-in, and exits with 1 when any of them is not ready. The addresses come from `QDRANT_URL` (the gRPC port, default `http://localhost:6334`) and `FALKORDB_URL` (default `falkor://localhost:6379`). The environment wins over `.env`.

If another program on your machine, such as a Homebrew Redis, already listens on port 6379, `localhost:6379` reaches that program and not the FalkorDB container. The FalkorDB line then fails with "did not answer like FalkorDB". Stop the other program or map the container to another port, and set `FALKORDB_URL` to match.

## Ingesting a chapter

```bash
cargo run -p rag-ingestion --bin rag-ingest -- samples/content/quanty-sample-notes/chapter-2
```

Reads one converted chapter folder with `ocr::read_chapter` and stores its items in a Qdrant collection, `items` unless `QDRANT_ITEMS_COLLECTION` names another. Text becomes chunks of about 300 to 500 tokens that never run past a heading, and a paragraph that a page break cut in two is joined again. Each formula, figure and table is an item of its own. A figure is embedded as its own picture together with its explanation, as one vector, and its payload keeps the path of the picture. Every item carries the book, chapter and section it sits in.

The ids are computed from the chapter's source hash and the place of the item in the chapter, so running the command again on the same chapter overwrites the same points and adds none. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each run by the token.

The command also writes the chapter to the FalkorDB graph, `quanty` unless `FALKORDB_GRAPH` names another: one `Document` node, one `Item` node for each item, an edge `HAS_ITEM` from the document to each item, and an edge `NEXT` from each item to the one after it in reading order. An `Item` node has the same id as its point in Qdrant. The collection is prepared and the graph is written before anything is embedded, so a store that is down fails the run before Gemini bills anything, and a run that stopped half way is finished by running it again. A second run on the same chapter adds no node and no edge.

Last, after the points are stored, the command asks `claude` which concepts each item discusses and how they relate, four items at a time. Every item is asked about, a figure as its explanation with no picture. `claude` runs on your subscription with the `haiku` model, with no tools and in safe mode. It is not started while `ANTHROPIC_API_KEY` is set, so that the work is not billed to the API: the chapter is stored, and then the run stops with a message that says to unset the key. The prompt and the JSON Schema of the answer are in `crates/rag-ingestion/src/ingest/concepts/prompts/`.

The answers are written to the graph. A `Concept` node has an id, a name, a one-line definition and a list of aliases, and it belongs to no document. An edge `MENTIONS` goes from an item to each concept it discusses, with the wording the item used. An edge `RELATES_TO` goes from one concept to another, with one of the types `DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF` and `USED_FOR`, and the item that stated it. A name is matched by its exact normalised form: lower case, with every run of punctuation or hyphens turned into one space. So "Black–Scholes model" and "black-scholes model" are one concept, and a concept named by two items is one node with two `MENTIONS`. Nothing else is matched yet. A relation is dropped and counted when it names a concept that neither its own answer nor the graph holds, or when it joins a concept to itself.

Every good answer is kept as a file in the folder that `CONCEPT_CACHE_DIR` names, `data/concept-cache` unless set. The name of the file is made from the prompt, its version, the schema, the model and the text that was sent, so a second run on the same chapter makes no `claude` call and adds no node and no edge. A usage limit, a missing sign-in, or a `claude` program that cannot be started stops the run with a clear message and is never tried again: the chapter is stored and can be searched, and running the same command again goes on from the answers that are kept. Any other failure of one item is tried once more, and then the item is skipped, not kept, and named in the summary, and the run goes on. The next run asks about it again.

After the lines about items and points, the summary prints how many concepts were created and how many were linked to an existing one, the mentions written, the relations written and dropped, the `claude` calls made and the cache hits, and the number of items skipped with one line for each.

## Deleting a document

```bash
cargo run -p rag-ingestion --bin rag-ingest -- delete-document <document id>
```

Removes one document from both stores: its points from the Qdrant collection, and its `Document` node, its `Item` nodes and all their edges from the graph, the `MENTIONS` of its items among them. Other documents are left whole. Concepts and their `RELATES_TO` edges stay, because they belong to no document. The document id is the one that an ingest prints as `document id`. The command prints how many points and nodes it removed, and refuses an id under which neither store holds anything, so an item id or a mistyped id removes nothing.

Run it again if it stopped half way: it removes what is left. It is also the way to clear a chapter before it is ingested again after the way it is cut into items has changed, because an ingest never removes the points and nodes of an earlier run.

## Asking a question

```bash
cargo run -p rag-retrieval --bin rag-query -- "What is the Black–Scholes partial differential equation?"
cargo run -p rag-retrieval --bin rag-query -- eval
```

The first command embeds the question, takes the five nearest items from the collection that `QDRANT_ITEMS_COLLECTION` names (`items` unless set) and prints each one with the title of its document, its page (the printed page number where there is one), its kind, its score and its text. A figure also prints the path of its own picture. `--kind` looks only at items of one kind: `chunk`, `formula`, `figure` or `table`, and any other value is refused. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each question by the token.

`golden.toml` holds 20 questions about the three sample chapters in `samples/content`, each with the document and the page whose items must come back. `eval` asks them all and prints one line for each, then a last line with the score, in the shape `found in the top 5: <found> of 20`. The score is one number that can be compared from run to run, so a change to retrieval must not lower it. Run `eval` from the workspace root, where `golden.toml` is, after ingesting the three sample chapters.

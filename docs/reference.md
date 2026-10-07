# quanty reference

Every command in full detail. For the short version, see the [README](../README.md).

PDFs are turned into searchable items (text, LaTeX formulas and figures). An LLM extracts the concepts each item mentions. Concepts are global, so two PDFs that both discuss Black–Scholes attach to the same node, and every new PDF links itself to everything already ingested with no manual work.

## How it works

- **Qdrant** stores every vector (items and concepts).
- **FalkorDB** stores the graph: documents, items and concepts, joined by `HAS_ITEM`, `NEXT`, `MENTIONS` and `RELATES_TO` edges.
- **Gemini Embedding 2** embeds text, LaTeX and images into one vector space.
- **Claude** (via `claude -p`) extracts and resolves concepts, and writes answers.
- Retrieval finds the closest items by vector search, then follows concepts across documents to pull in related material.

## Crates

| Crate | Purpose |
| --- | --- |
| `ocr` | PDF to text, LaTeX, figures and figure descriptions |
| `rag-core` | Shared types, IDs, config, `Embedder` and `Llm` traits, Qdrant wrapper |
| `graph` | `GraphStore` trait and FalkorDB implementation |
| `rag-ingestion` | Chunking, embedding, concept extraction and resolution (`rag-ingest`) |
| `rag-retrieval` | Vector search, graph expansion, ranking, answers with citations (`rag-query`) |
| `gui` | The desktop app: a question, its answer with citations, the page each one stands on, the concept graph and the path of the search (`quanty`) |
| `mcp` | An MCP server, so that an AI agent can search, read pages, get answers and ingest a PDF (`quanty-mcp`) |

## Getting started

```bash
cp .env.example .env   # add your API keys
docker compose up -d   # Qdrant + FalkorDB
```

The `claude` CLI must be on your `PATH` and signed in.

## Converting a chapter

```bash
cargo run --release -p ocr -- --book "Option Volatility and Pricing" chapter-1-sample-pages.pdf
```

The file must be named `chapter-<number>-<name>.pdf`. Every page is broken into its pieces (headings, text, formulas, figures, tables and footnotes) and saved under `content/<book-title>/chapter-<number>/page-num-<i>/`, with a `page.json` for each page and a `chapter.json` for the chapter. Each figure also gets its own picture, `NN-figure.png`, cut out of the page beside its `.md` file, and a figure that cannot be cut out keeps the whole page as its picture and is listed in the summary. `--out <folder>` saves somewhere other than `content`. The command only saves files: nothing is put into Qdrant or FalkorDB.

The command reads `CONVERTER_JEV_API_KEY` from `.env`, and it refuses to run while `ANTHROPIC_API_KEY` is set, so the work is billed to the `claude` subscription. A second run on a finished chapter makes no outside call and costs nothing. An interrupted run picks up at the pages that are missing. A different PDF for the same chapter is refused.

Later crates read a chapter back, in reading order, with `ocr::read_chapter`.

## Checking the stores

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

Prints one line each for Qdrant, FalkorDB and the `claude` sign-in, and exits with 1 when any of them is not ready. The addresses come from `QDRANT_URL` (the gRPC port, default `http://localhost:6334`) and `FALKORDB_URL` (default `falkor://localhost:6379`). The environment wins over `.env`.

If another program on your machine, such as a Homebrew Redis, already listens on port 6379, `localhost:6379` reaches that program and not the FalkorDB container. The FalkorDB line then fails with "did not answer like FalkorDB". Stop the other program or map the container to another port, and set `FALKORDB_URL` to match.

## Ingesting a chapter

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- samples/content/quanty-sample-notes/chapter-2
```

Reads one converted chapter folder with `ocr::read_chapter` and stores its items in a Qdrant collection, `items` unless `QDRANT_ITEMS_COLLECTION` names another. Text becomes chunks of about 300 to 500 tokens that never run past a heading, and a paragraph that a page break cut in two is joined again. Each formula, figure and table is an item of its own. A figure is embedded as its own picture together with its explanation, as one vector, and its payload keeps the path of the picture. Every item carries the book, chapter and section it sits in.

A formula, a figure or a table that has a printed label, such as "(7.3)", "Figure 13-4" or "Table 1-1", stores it in the payload as `label`. A chunk stores the labels of the figures, tables and equations that its text points at as `cites`, each once, in reading order. A footnote marker is not one of them. Both fields are left out of the payload when there is nothing to store. A chapter that was stored before these two fields existed does not have them, so ingest it again to add them. `delete-document` is not needed for this: the ids are the same, so the points are written over.

The ids are computed from the chapter's source hash and the place of the item in the chapter, so running the command again on the same chapter overwrites the same points and adds none. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each run by the token.

The command also writes the chapter to the FalkorDB graph, `quanty` unless `FALKORDB_GRAPH` names another: one `Document` node, one `Item` node for each item, an edge `HAS_ITEM` from the document to each item, and an edge `NEXT` from each item to the one after it in reading order. An `Item` node has the same id as its point in Qdrant. The collection is prepared and the graph is written before anything is embedded, so a store that is down fails the run before Gemini bills anything, and a run that stopped half way is finished by running it again. A second run on the same chapter adds no node and no edge. The `Document` node also has the property `ingested_items`, the number of its items, which is the mark that the document is ingested whole. An ingest takes the mark away when it starts and sets it as its last step, only when no item was skipped. Only `rag-ingest pdf` reads it, as "Ingesting a whole chapter PDF" describes.

Last, after the points are stored, the command asks `claude` which concepts each item discusses and how they relate, four items at a time. Every item is asked about, a figure as its explanation with no picture. An item that is alone in its document, such as a picture, is asked about together with the five stored items nearest to it from other documents, under the heading "Possibly related material", so that the model names its concepts as they are named elsewhere. The model is told to name only what the item itself shows. The kept answer does not depend on that material, so an item is asked about once. `claude` runs on your subscription with the `haiku` model, with no tools and in safe mode. It is not started while `ANTHROPIC_API_KEY` is set, so that the work is not billed to the API: the chapter is stored, and then the run stops with a message that says to unset the key. The prompt and the JSON Schema of the answer are in `crates/rag-ingestion/src/ingest/concepts/prompts/`.

The answers are written to the graph. A `Concept` node has an id, a name, a one-line definition and a list of aliases, and it belongs to no document. An edge `MENTIONS` goes from an item to each concept it discusses, with the wording the item used. An edge `RELATES_TO` goes from one concept to another, with one of the types `DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF` and `USED_FOR`, and the item that stated it. A concept named by two items is one node with two `MENTIONS`. A relation is dropped and counted when it names a concept that neither its own answer nor the graph holds, or when it joins a concept to itself.

A name is matched to a stored concept in four steps, one concept at a time, so that a name an earlier item made is seen by a later one:

1. The name is compared with the name and the aliases of every stored concept in its normalised form: lower case, with every run of punctuation or hyphens turned into one space. So "Black–Scholes model" and "black-scholes model" are one concept. A match is linked and nothing else is written.
2. When nothing matches, the name and its definition are embedded as "name: definition", and the nearest stored concept is looked up in a second Qdrant collection. It is `concepts` unless `QDRANT_CONCEPTS_COLLECTION` names another, and it holds one point for each concept, under the id of the concept's node, with the name and the aliases as its payload.
3. A cosine score of 0.95 or more links the name to that concept and adds the name to its aliases, in the node and in the point. A score under 0.75 makes a new concept, written to the graph and to the collection. Between the two, `claude` is asked whether the two concepts are the same, with both names and both definitions: yes links and adds the alias, no makes a new concept. Only the nearest concept is compared, and the two thresholds are the constants `LINK_SCORE` and `ASK_SCORE` of `crates/rag-ingestion/src/ingest/concepts/resolve.rs`.
4. The `MENTIONS` edge is written with the wording the item used. When two names of one answer end as one concept, the item mentions it once, with the first wording.

Every decision is added to a log, `data/concept-decisions.jsonl` unless `CONCEPT_DECISION_LOG` names another file, as one line of JSON. A line has `item` (the id of the item that named the concept), `name` (as the item wrote it) and `rule`: `exact-name`, `high-score`, `llm-same`, `llm-different`, `low-score` or `nothing-stored`. Where they apply it also has `matched` (the stored concept the name was linked to, with its `id` and `name`), `nearest` (the nearest stored concept, which was turned down), `score` and `created` (the id of the new concept). A line is added after the writes it led to have succeeded.

If `claude` fails twice to say whether two concepts are the same, the run stops with a message that names both and says to run the same command again. Nothing is guessed: a guess of "different" would make a second concept that nothing can merge, and a guess of "yes" would make a wrong alias that every later item matches. The items are stored and what was linked so far stays, and the next run goes on from there. A usage limit stops the run in the same way.

Every good answer is kept as a file in the folder that `CONCEPT_CACHE_DIR` names, `data/concept-cache` unless set. The name of the file is made from the prompt, its version, the schema, the model and the text that was sent, so a second run on the same chapter makes no `claude` call and adds no node, no edge, no concept point and no alias: every name is now found in step 1, so no concept is embedded and no comparison is asked. The answer to a question about two concepts is kept in the same folder, and the summary counts these questions with the others. A usage limit, a missing sign-in, or a `claude` program that cannot be started stops the run with a clear message and is never tried again: the chapter is stored and can be searched, and running the same command again goes on from the answers that are kept. Any other failure of one item is tried once more, and then the item is skipped, not kept, and named in the summary, and the run goes on. The next run asks about it again.

After the lines about items and points, the summary prints how many concepts were created and how many were linked to an existing one, the mentions written, the relations written and dropped, the `claude` calls made and the cache hits, and the number of items skipped with one line for each.

## Ingesting a whole chapter PDF

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf --book "Option Volatility and Pricing" chapter-1-sample-pages.pdf
```

Does in one run what "Converting a chapter" and "Ingesting a chapter" do in two, with no one in between. The file must be named `chapter-<number>-<name>.pdf`, and `--book` is the title of the book that the chapter is from. A missing `--book`, a file name that does not fit and a path that is not there are each refused with a message before anything is started, and cost nothing. The PDF is converted by `ocr` into the folder that `CONTENT_DIR` names (`content` unless set), under `<book-title>/chapter-<number>/`, exactly as in "Converting a chapter". The converted chapter folder is then ingested with the steps of "Ingesting a chapter".

The command needs `CONVERTER_JEV_API_KEY` for the conversion and `EMBEDDING_GEMINI_API_KEY` for the embedding, each from the environment or from `.env`. A conversion is not started while `ANTHROPIC_API_KEY` is set, so that the work is billed to the `claude` subscription and not to the API. The embedder is set up and both stores are checked before the first page is converted, so a missing Gemini key or a store that is down stops the run before any page is paid for.

It prints one summary: the summary that `ocr` prints (the pages processed, with how many were converted now and how many were already done, the routes, the pieces, the calls and the pages to check), then the lines that an ingest of a chapter folder prints, which count the items written by kind, the concepts created and the concepts linked to existing ones.

A page that fails stops the run, and the message names its page number. The pages that were converted stay saved, so the same command goes on from the pages that are missing and then ingests the chapter. A run that stops during the ingest, for example at a usage limit of `claude`, is finished the same way: the same command converts nothing more and goes on from the answers that are kept.

A second run on the same PDF prints `already ingested` with the document id and the number of items. It makes no `ocr`, embedding or `claude` call and writes nothing to either store, except the labels of "Labelling a document" when `--author` or `--tag` is given. The document id is made from the SHA-256 of the PDF, so the PDF is the same document under any file name. The command takes a document as ingested when both stores agree: the `Document` node in the graph has the mark `ingested_items` with a number `n`, and the Qdrant collection holds exactly `n` points of the document. The count of points is what stops the mark from being believed after a collection was emptied or changed, or after a `delete-document` that stopped half way.

The mark is set by every ingest, of a chapter folder, of a picture or by `pdf`, and only as its last step, so it is never there for a run that stopped half way. An ingest in which an item was skipped does not set it either, so the next run goes on from there. This also means that a document with an item that fails on every run is never marked: each `pdf` run on it converts nothing again, but ingests it again, and embeds it again. The points of an earlier cut of a chapter stay in the collection after the way the chapter is cut has changed, so their count is larger than the mark and `pdf` ingests the chapter again on each run; run `delete-document` first. A document that was ingested before the mark existed has none, so the first `pdf` run on it ingests it once more.

`rag-ingest <chapter folder>` never reads the mark. It always embeds and stores again, so it is the way to ingest a document that is already stored once more.

## Ingesting a lone picture

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- chart.png --note "A chart from a book on option trading."
```

A picture that stands alone, such as a chart, is ingested as a document of one figure. The file must be a PNG or a JPEG. `ocr` copies it to `images/<the first 16 hex digits of its SHA-256>/` under the folder that `CONTENT_DIR` names (`content` unless set), asks Sonnet through `claude` to explain it, and saves the explanation, which quotes the words printed on the figure, as `figure.md`, with an `image.json` that says where it came from. Nothing is cut out of the picture. It is not started while `ANTHROPIC_API_KEY` is set, and the command checks `EMBEDDING_GEMINI_API_KEY` before Sonnet is paid. The command then does what it does for a chapter: one `Document` node, named after the file, and one `Item` node, one point that is embedded from the picture together with its explanation, and the concepts of the explanation.

`--note` is what you know about the picture. It is added to the explanation as a last line, `Note: …`, before the picture is embedded and before concepts are asked for, so the stored text of the item ends with it. A chapter folder takes no note.

The document id is made from the SHA-256 of the picture's bytes, as a chapter's is made from its PDF, so the same picture under another name is the same document, and the note does not change it. A second run on the same picture calls neither Sonnet nor `claude`, because the converted files and the answers are kept; Gemini embeds the picture again, as it does for a chapter. The note is part of the item, so to give a picture another note, delete its document first with `delete-document`: otherwise the mentions of the earlier note stay beside the new ones.

## Labelling a document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- samples/content/option-volatility-and-pricing/chapter-1 --author "Sheldon Natenberg" --tag options --tag volatility
cargo run --release -p rag-ingestion --bin rag-ingest -- tag <document id> --author "Sheldon Natenberg" --add greeks --remove volatility
```

A document has three labels: its `book`, its `author` and free `tags`, such as "options". They belong to the document, so every item point of it stores them flat in its payload, as `book`, `author` and `tags`, and the `Document` node has the same three properties. A label that the document does not have is left out, except that the node of a document with no tag keeps `tags` as an empty list. Labels are not embedded and no id is made from them, so a label never changes a document id, an item id or the text that Gemini embeds. A tag is not a concept: nothing in the concept graph changes.

The book is the title that the chapter was converted under, so it is set by the ingest, and a lone picture has none. The author and the tags are given with `--author "<name>"` and a `--tag <tag>` that can be repeated, on all three ingest commands: a chapter folder, a lone picture and `pdf`. A tag is stored in lower case with no space at either end, and each tag once, so `Options` and ` options ` are one tag. An empty author or tag is refused. The book and the author are stored as they were given.

The book is written by the ingest itself, and the author and the tags that were given are written after the ingest has finished. A run that stops in the ingest writes neither the author nor the tags, and the same command again finishes the ingest and writes them. `--author` replaces the author, and `--tag` adds a tag and never takes one away. An ingest never removes a label: ingesting a chapter again with no `--author` and no `--tag` keeps the author and the tags that the document has.

`tag` changes the labels of a document that is stored, in both stores, with no embedding and no call of any model. `--author` sets the author, each `--add` adds a tag, and each `--remove` takes a tag away, after the tags to add were added. It prints the labels that the document has afterwards. The document id is the one that an ingest prints as `document id`, and an id under which the graph holds no document is refused and changes nothing. `pdf` on a document that is already ingested, given `--author` or `--tag`, converts, embeds and asks nothing, and writes those labels the same way.

A document that was stored before labels existed has no `book` until it is ingested again, which embeds it again and is billed by Gemini. Its author and tags need no embedding: set them with `tag`.

## Deleting a document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- delete-document <document id>
```

Removes one document from both stores: its points from the Qdrant collection, and its `Document` node, its `Item` nodes and all their edges from the graph, the `MENTIONS` of its items among them. Other documents are left whole. Concepts, their `RELATES_TO` edges, their points in the concepts collection and their aliases stay, because they belong to no document. The document id is the one that an ingest prints as `document id`. The command prints how many points and nodes it removed, and refuses an id under which neither store holds anything, so an item id or a mistyped id removes nothing.

Run it again if it stopped half way: it removes what is left. It is also the way to clear a chapter before it is ingested again after the way it is cut into items has changed, because an ingest never removes the points and nodes of an earlier run.

## Asking a question

```bash
cargo run --release -p rag-retrieval --bin rag-query -- "What is the Black–Scholes partial differential equation?"
cargo run --release -p rag-retrieval --bin rag-query -- --answer "What is the Black–Scholes partial differential equation?"
```

The command reads what an ingest wrote: the items collection that `QDRANT_ITEMS_COLLECTION` names (`items` unless set), the concepts collection that `QDRANT_CONCEPTS_COLLECTION` names (`concepts` unless set) and the graph that `FALKORDB_GRAPH` names (`quanty` unless set). Every ingest creates both collections, also when it finds no concept. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each question by the token. A store that is down stops the command with a message that names it. FalkorDB is connected to before the question is embedded, so a graph that is down costs nothing.

The search has six steps, in this order:

1. The question is embedded, and the 8 items nearest to it are the seeds.
2. The seed concepts are the 3 concepts nearest to the question, and the concepts that the seeds mention.
3. The graph adds the items that mention a seed concept, and the concepts that are one `RELATES_TO` edge from a seed concept, with the items that mention those. The graph adds at most 50 items.
4. The seeds and the added items are ranked together by one more query to Qdrant that looks only at those items, so they are all scored by the same cosine to the question.
5. No document gives more than 3 results, and the first 8 results that are left are kept.
6. When no `--kind` is given, each figure, table and equation that a result cites by its printed label, such as "Figure 13-4", "Table 7-2" or "(7.3)", is added if it is in the same document and is not in the results yet.

The four numbers of these steps are constants in `crates/rag-retrieval/src/search/mod.rs`. The two that matter most are `RESULTS_PER_QUERY` (8) and `MAX_RESULTS_PER_DOCUMENT` (3). All four are starting values. A document gives at most three results in steps 1 to 5, also when it is the only document in the store. An item that is not a seed never scores above a seed, so an item that came in through the graph is shown only where the cap left a place free. When no document has more than three of the eight nearest items, no result comes in through the graph. With no concept in the graph, the results are the nearest items only.

Each result prints the title of its document, its page (the printed page number where there is one), its kind, its label where it has one, its score and its text. A figure also prints the path of its own picture. Two notes can follow the score:

- `reached via concept <name>` marks an item that came in through the graph. The name is the first of the concepts, in the order of the steps above, that the item mentions.
- `cited by result <n> as <label>` marks an item that was added in step 6 because result `n` cites it.

The items of step 6 come after the others. They are not counted in the 8 or in the 3 of their document, and they are not in score order with the others. A citation finds only the label as it is printed, in the same document: "Fig. 7-2" does not find "Figure 7-2". `--kind` looks only at items of one kind: `chunk`, `formula`, `figure` or `table`, and any other value is refused. With `--kind` nothing is added in step 6.

`--book "<title>"`, `--author "<name>"` and `--tag <tag>` look only at items of documents that have those labels, as "Labelling a document" describes them. Each is optional and `--tag` can be repeated. A document must match everything that is given: the book and the author match whatever their capitals, and the document must have every tag given. The filter holds for every item that is printed, the nearest items, the items that the graph adds and the items of step 6, and it works with `--kind` and with `--answer`. When no document matches, the command prints `no items found` and the question is not embedded.

`--answer` asks Sonnet, `claude-sonnet-5-5`, to write an answer from the items that were found. It runs through the `claude` command on your subscription, with no tools, and it is not started while `ANTHROPIC_API_KEY` is set. It makes one call. The model is given the question and the items, numbered: the document, the page, the kind, the label, and the text, the raw LaTeX of a formula, or the explanation of a figure with the path of its picture. It replies with claims, and each claim names the numbers of the items that support it. The command prints only the answer, not the results, and what the call cost goes to standard error. After each claim it prints one line for each source: the title of the document, which holds the chapter, the printed page, and the kind and label of the item. Under the line of a formula it prints the LaTeX exactly as the document has it, and under the line of a figure it prints the path of the picture. The program writes the citations and the LaTeX, not the model, so a title, a page or a formula is never retyped. A reply with a claim that names no source, or an item that was not given, is an error, and the question is not asked again. When nothing is found, the command prints `no items found` and does not ask the model. When the model replies with no claim, the command prints `the stored items do not answer the question`. The prompt and the JSON Schema of the reply are in `crates/rag-retrieval/src/answer/prompts/`.

## The desktop app

```bash
cargo run --release -p gui
```

The program is `quanty`. It opens one window, the Ask screen: the question with its mode and its book, author and tag filters; the answer with its citations, and the results by kind; the page that a citation stands on, with its figures, formulas, tables and concepts; the concept graph; the steps of the search; and questions to ask next. The keys are listed in the [README](../README.md).

It needs what `rag-query --answer` needs: Qdrant and FalkorDB, `EMBEDDING_GEMINI_API_KEY`, and `claude` signed in with `ANTHROPIC_API_KEY` not set. An ask makes one embedding call and one Sonnet call. In the mode **Results only** it makes the embedding call alone. When a service is not ready, the part of the window that needed it says which one and what to do.

The program reads `.env`, `content/` and `data/` from its home folder. It finds that folder in this order: the folder given with `--home <folder>`; the folder that `QUANTY_HOME` names; the nearest folder at or above the current one that holds a `.env`; the nearest folder at or above the program's own folder that holds one, which is what a start from Finder uses; and last the current folder, with no `.env`.

A citation opens its page when the chapter's folder is found. The chapter is looked for under the content folder, by its source file; a folder that is stored with the document is used first, when there is one. A chapter that is found in neither way shows "The page was not found" with what to do: this is what a chapter that was ingested from a folder outside the content folder shows.

`--fixture <scene>` runs the whole window on built-in data from `samples/content`, with no store, no model and no cost. `--fixture list` prints the scenes: each is one state of the screen, such as `black-scholes` (a full answer), `stores-down` or `first-run`.

The **Ingest** tab adds one chapter to the library: choose a chapter PDF named `chapter-<number>-<name>.pdf`, choose the book or add a new one, check the chapter, then start. The check is free; a start is paid work, the same as `rag-ingest pdf`.

Not built yet: the Library page, the notices tray, the help sheet and the health check. Labels are changed and documents are deleted with `rag-ingest`.

## The MCP server

`quanty-mcp` lets an AI agent search the stored books, read their pages, get a cited answer and send a chapter PDF to be ingested, over the Model Context Protocol. How to build and start it, its tools and their costs are in [mcp.md](mcp.md).

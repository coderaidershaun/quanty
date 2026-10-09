# quanty reference

Every command in full detail. For the short version, see the [README](../README.md).

PDFs are turned into searchable items (text, LaTeX formulas and figures). An LLM extracts the concepts each item mentions. Concepts are global, so two PDFs that both discuss Black–Scholes attach to the same node, and every new PDF links itself to everything already ingested with no manual work.

## How it works

- **Qdrant** stores every vector (items and concepts).
- **FalkorDB** stores the graph: documents, items and concepts, joined by `HAS_ITEM`, `NEXT`, `MENTIONS` and `RELATES_TO` edges, and one `Media` node for each media, with its title, category, authors and tags. A `Media` node has no edge: a document belongs to the media whose title it carries, whatever the capitals.
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

## Converting a book's chapter

```bash
cargo run --release -p ocr -- --book "Option Volatility and Pricing" samples/chapter-1-sample-pages.pdf
```

`ocr` converts the PDF of a book's chapter. The PDF of a paper or another media is converted by `rag-ingest pdf` (see "Ingesting a PDF"). The file must be named `chapter-<number>-<name>.pdf`, and `--book` is the title of the book. Every page is broken into its pieces (headings, text, formulas, figures, tables and footnotes) and saved under `content/<media folder>/chapter-<number>/page-num-<i>/`, with a `page.json` for each page and a `chapter.json` for the document. Each figure also gets its own picture, `NN-figure.png`, cut out of the page beside its `.md` file, and a figure that cannot be cut out keeps the whole page as its picture and is listed in the summary. `--out <folder>` saves somewhere other than `content`. The command only saves files: nothing is put into Qdrant or FalkorDB.

The command reads `CONVERTER_JEV_API_KEY` from `.env`, and it refuses to run while `ANTHROPIC_API_KEY` is set, so the work is billed to the `claude` subscription. A second run on a finished chapter makes no outside call and costs nothing. An interrupted run picks up at the pages that are missing. A different PDF for the same chapter is refused.

The summary at the end counts the pages, the routes, the pieces and the paid calls of this run, and gives one line for each model that answered: `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write`. It gives no cost, because `ocr` has no table of prices: `rag-ingest pdf` gives the cost of a conversion. See "Costs".

Every way of converting a PDF uses the same folders. The media folder is the title of the media in lower case, with one dash for each run of characters that are not letters or digits, so "Option Volatility and Pricing" is `option-volatility-and-pricing`. In it, each document has a folder of its own:

| Document | Folder |
| --- | --- |
| A chapter of a book | `<content>/<media folder>/chapter-<number>/` |
| The PDF of a paper or another media | `<content>/<media folder>/<document title folder>/`, the title of the document made into a folder name the same way |

`chapter.json` says what the folder holds: `media-title`, `name` (`{"chapter": {"number": …, "name": …}}` for a chapter, `{"title": …}` for a document with a title of its own), `source-file`, `source-sha256` (the SHA-256 of the PDF), `page-count` and `finished`.

Later crates read a converted document back, in reading order, with `ocr::read_chapter`.

## Checking the stores

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

Prints one line each for Qdrant, FalkorDB and the `claude` sign-in, and exits with 1 when any of them is not ready. The addresses come from `QDRANT_URL` (the gRPC port, default `http://localhost:6334`) and `FALKORDB_URL` (default `falkor://localhost:6379`). The environment wins over `.env`.

If another program on your machine, such as a Homebrew Redis, already listens on port 6379, `localhost:6379` reaches that program and not the FalkorDB container. The FalkorDB line then fails with "did not answer like FalkorDB". Stop the other program or map the container to another port, and set `FALKORDB_URL` to match.

## Ingesting a converted document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- samples/content/quanty-sample-notes/chapter-2
```

Reads one converted document folder, of a book's chapter or of any other document, with `ocr::read_chapter` and stores its items in a Qdrant collection, `items` unless `QDRANT_ITEMS_COLLECTION` names another. Text becomes chunks of about 300 to 500 tokens that never run past a heading, and a paragraph that a page break cut in two is joined again. Each formula, figure and table is an item of its own. A figure is embedded as its own picture together with its explanation, as one vector, and its payload keeps the path of the picture. Every item carries the title of its document and the section it sits in.

A formula, a figure or a table that has a printed label, such as "(7.3)", "Figure 13-4" or "Table 1-1", stores it in the payload as `label`. A chunk stores the labels of the figures, tables and equations that its text points at as `cites`, each once, in reading order. A footnote marker is not one of them. Both fields are left out of the payload when there is nothing to store. A document that was stored before these two fields existed does not have them, so ingest it again to add them. `delete-document` is not needed for this: the ids are the same, so the points are written over.

The ids are computed from the document's source hash and the place of the item in the document, so running the command again on the same document overwrites the same points and adds none. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each run by the token.

The command also writes the document to the FalkorDB graph, `quanty` unless `FALKORDB_GRAPH` names another: one `Document` node, one `Item` node for each item, an edge `HAS_ITEM` from the document to each item, and an edge `NEXT` from each item to the one after it in reading order. An `Item` node has the same id as its point in Qdrant. The collection is prepared and the graph is written before anything is embedded, so a store that is down fails the run before Gemini bills anything, and a run that stopped half way is finished by running it again. A second run on the same document adds no node and no edge. The `Document` node also has the property `ingested_items`, the number of its items, which is the mark that the document is ingested whole. An ingest takes the mark away when it starts and sets it as its last step, only when no item was skipped. Only `rag-ingest pdf` reads it, as "Ingesting a PDF" describes.

The media of the document is the one that `chapter.json` names. `--category` (`book`, the default, `paper` or `other`), `--author`, given once for each author, and `--tag`, given once for each tag, label that media. They are used only when the library does not have the media yet, because an ingest never changes a stored media: see "Labelling a media and a document". `--doc-tag`, given once for each tag, adds own tags to the document after the ingest has finished.

Last, after the points are stored, the command asks `claude` which concepts each item discusses and how they relate, four items at a time. Every item is asked about, a figure as its explanation with no picture. An item that is alone in its document, such as a picture, is asked about together with the five stored items nearest to it from other documents, under the heading "Possibly related material", so that the model names its concepts as they are named elsewhere. The model is told to name only what the item itself shows. The kept answer does not depend on that material, so an item is asked about once. `claude` runs on your subscription with the `haiku` model, with no tools and in safe mode. It is not started while `ANTHROPIC_API_KEY` is set, so that the work is not billed to the API: the run stops before anything is embedded or stored, with a message that says to unset the key. The prompt and the JSON Schema of the answer are in `crates/rag-ingestion/src/ingest/concepts/prompts/`.

The answers are written to the graph. A `Concept` node has an id, a name, a one-line definition and a list of aliases, and it belongs to no document. An edge `MENTIONS` goes from an item to each concept it discusses, with the wording the item used. An edge `RELATES_TO` goes from one concept to another, with one of the types `DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF` and `USED_FOR`, and the item that stated it. A concept named by two items is one node with two `MENTIONS`. A relation is dropped and counted when it names a concept that neither its own answer nor the graph holds, or when it joins a concept to itself.

A name is matched to a stored concept in four steps, one concept at a time, so that a name an earlier item made is seen by a later one:

1. The name is compared with the name and the aliases of every stored concept in its normalised form: lower case, with every run of punctuation or hyphens turned into one space. So "Black–Scholes model" and "black-scholes model" are one concept. A match is linked and nothing else is written.
2. When nothing matches, the name and its definition are embedded as "name: definition", and the nearest stored concept is looked up in a second Qdrant collection. It is `concepts` unless `QDRANT_CONCEPTS_COLLECTION` names another, and it holds one point for each concept, under the id of the concept's node, with the name and the aliases as its payload.
3. A cosine score of 0.95 or more links the name to that concept and adds the name to its aliases, in the node and in the point. A score under 0.75 makes a new concept, written to the graph and to the collection. Between the two, `claude` is asked whether the two concepts are the same, with both names and both definitions: yes links and adds the alias, no makes a new concept. Only the nearest concept is compared, and the two thresholds are the constants `LINK_SCORE` and `ASK_SCORE` of `crates/rag-ingestion/src/ingest/concepts/resolve.rs`.
4. The `MENTIONS` edge is written with the wording the item used. When two names of one answer end as one concept, the item mentions it once, with the first wording.

Every decision is added to a log, `data/concept-decisions.jsonl` unless `CONCEPT_DECISION_LOG` names another file, as one line of JSON. A line has `item` (the id of the item that named the concept), `name` (as the item wrote it) and `rule`: `exact-name`, `high-score`, `llm-same`, `llm-different`, `low-score` or `nothing-stored`. Where they apply it also has `matched` (the stored concept the name was linked to, with its `id` and `name`), `nearest` (the nearest stored concept, which was turned down), `score` and `created` (the id of the new concept). A line is added after the writes it led to have succeeded.

If `claude` fails twice to say whether two concepts are the same, the run stops with a message that names both and says to run the same command again. Nothing is guessed: a guess of "different" would make a second concept that nothing can merge, and a guess of "yes" would make a wrong alias that every later item matches. The items are stored and what was linked so far stays, and the next run goes on from there. A usage limit stops the run in the same way.

Every good answer is kept as a file in the folder that `CONCEPT_CACHE_DIR` names, `data/concept-cache` unless set. The name of the file is made from the prompt, its version, the schema, the model and the text that was sent, so a second run on the same document asks `claude` no question and adds no node, no edge, no concept point and no alias: every name is now found in step 1, so no concept is embedded and no comparison is asked. The answer to a question about two concepts is kept in the same folder, and the summary counts these questions with the others. Every run first checks that `ANTHROPIC_API_KEY` is not set and runs `claude auth status` once, which is free and asks no question. A set key, a missing sign-in, or a `claude` program that cannot be started stops the run there, with `claude cannot be asked for the concepts of the document, so nothing was embedded or stored` and the reason. A usage limit, or a sign-in that is lost during the run, stops the run with a clear message and is never tried again: the document is stored and can be searched, and running the same command again goes on from the answers that are kept. Any other failure of one item is tried once more, and then the item is skipped, not kept, and named in the summary, and the run goes on. The next run asks about it again.

After the lines about items and points, the summary prints how many concepts were created and how many were linked to an existing one, the mentions written, the relations written and dropped, the `claude` calls made and the cache hits, and the number of items skipped with one line for each. Last come one line for each model that the run used, `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write`, with ` (estimated)` after the embedding, whose count is worked out, and `cost of this run: ≈ $<cost>`. A kept answer costs nothing. For a picture, the Sonnet call that explained it is one of the lines. See "Costs".

## Ingesting a PDF

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf --book "Option Volatility and Pricing" samples/chapter-1-sample-pages.pdf
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf --paper "Hawkes Processes in Finance" --author "First Author" --author "Second Author" --tag hawkes --doc-tag survey hawkes-notes.pdf
```

Does in one run what "Converting a book's chapter" and "Ingesting a converted document" do in two, with no one in between, for the PDF of any media.

| Flag | What it gives |
| --- | --- |
| `--book <title>`, `--paper <title>` or `--other <title>` | The title of the media, and its category. Give exactly one of the three |
| `--title <title>` | The title of the document, for a paper or another media. Without it, the document takes the title of the media. Not for a book |
| `--author <name>` | An author of the media. Give it once for each author, in order |
| `--tag <tag>` | A tag of the media. Give it once for each tag |
| `--doc-tag <tag>` | A tag of this document only. Give it once for each tag |

`--author` and `--tag` are used only when the library does not have the media yet, because an ingest never changes a stored media. The category of the flag is also used only to make a new media. When the library has a media of that title, whatever its capitals and the space at its ends, the stored category wins: it says how the PDF is named and which category the document carries. When it is not the category of the flag, the command says so in one line on standard error: `the library has "<title>" in the category <category>, so this pdf is named and labelled by that category, not by --<flag>`. To change the category of a stored media, use `rag-ingest media`.

The PDF of a book must be named `chapter-<number>-<name>.pdf`. The number and the name of the chapter come from the file name, with a capital letter for each word of the name, and the title of the document is "<book>, chapter <number>: <name>", such as "Option Volatility and Pricing, chapter 1: Sample Pages". A book takes no `--title`. The PDF of a paper or another media can have any name, and the title of its document is `--title`, or else the title of the media. A missing flag, a path that is not there, a `--title` for a book and a book's PDF with another name are each refused with a message before the first page is converted, and cost nothing. A book's PDF with another name is refused with `rename it to chapter-<number>-<name>.pdf` and the hint that a media the library does not have yet can take `--paper` or `--other`; when the library has the media as a book and the flag is `--paper` or `--other`, with `the library has "<title>" in the category book, so its pdf must be named chapter-<number>-<name>.pdf; rename <pdf>`.

The PDF is converted by `ocr` into the folder that `CONTENT_DIR` names (`content` unless set), in the folders of "Converting a book's chapter": `<media folder>/chapter-<number>/` for a book's chapter, and `<media folder>/<document title folder>/` for any other document. The converted folder is then ingested with the steps of "Ingesting a converted document", and the own tags of `--doc-tag` are written last.

The command needs `CONVERTER_JEV_API_KEY` for the conversion and `EMBEDDING_GEMINI_API_KEY` for the embedding, each from the environment or from `.env`. A conversion is not started while `ANTHROPIC_API_KEY` is set, so that the work is billed to the `claude` subscription and not to the API. The embedder is set up and both stores are checked before the first page is converted, so a missing Gemini key or a store that is down stops the run before any page is paid for. The graph is read before the PDF is named, because a stored media's category says how the PDF is named.

While it runs, the command prints one line on standard error for each step, so that standard output holds only the summary at the end:

| Line | When |
| --- | --- |
| `converting <n> of <total> pages (<done> already done)` | The pages are counted, and `<n>` of them are left to convert |
| `page <n> converted ($<cost>)` | A page is converted. The cost is its calls at the prices of "Costs", or `page <n> converted (cost unknown)` when a model of the page has no price |
| `page <n> failed` | A page failed |
| `writing the graph` | The document and its items are written to the graph |
| `embedding <n> items` | Gemini embeds the items |
| `storing the items` | The points are written to Qdrant |
| `reading the concepts of <n> items` | `claude` is asked which concepts the items discuss |
| `linking the concepts of <n> items` | The concepts are matched to the stored ones and linked |

A PDF that is already ingested prints none of these lines. The tokens and the seconds of each `claude` question of the concepts are also written to standard error.

The summary is the summary that `ocr` prints (the pages processed, with how many were converted now and how many were already done, the routes, the pieces, the calls, the tokens of each model of the conversion and the pages to check), then the lines that an ingest of a converted folder prints, which count the items written by kind, the concepts created and the concepts linked to existing ones, and the tokens of each model of the ingest. One line closes it: `cost of this run: ≈ $<cost>`, for the conversion and the ingest together.

A page that fails stops the run, and the message names its page number. The pages that were converted stay saved, so the same command goes on from the pages that are missing and then ingests the document. A run that stops during the ingest, for example at a usage limit of `claude`, is finished the same way: the same command converts nothing more and goes on from the answers that are kept.

A second run on the same PDF prints `already ingested` with the document id and the number of items. It makes no `ocr`, embedding or `claude` call and writes nothing to either store, except the own tags of `--doc-tag`. The document id is made from the SHA-256 of the PDF, so the PDF is the same document under any file name. The command takes a document as ingested when both stores agree: the `Document` node in the graph has the mark `ingested_items` with a number `n`, and the Qdrant collection holds exactly `n` points of the document. The count of points is what stops the mark from being believed after a collection was emptied or changed, or after a `delete-document` that stopped half way.

The mark is set by every ingest, of a converted folder, of a picture or by `pdf`, and only as its last step, so it is never there for a run that stopped half way. An ingest in which an item was skipped does not set it either, so the next run goes on from there. This also means that a document with an item that fails on every run is never marked: each `pdf` run on it converts nothing again, but ingests it again, and embeds it again. The points of an earlier cut of a document stay in the collection after the way the document is cut has changed, so their count is larger than the mark and `pdf` ingests the document again on each run; run `delete-document` first.

`rag-ingest <folder>` never reads the mark. It always embeds and stores again, so it is the way to ingest a document that is already stored once more.

## Ingesting a lone picture

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- chart.png --note "A chart from a book on option trading."
```

A picture that stands alone, such as a chart, is ingested as a document of one figure. The file must be a PNG or a JPEG. `ocr` copies it to `images/<the first 16 hex digits of its SHA-256>/` under the folder that `CONTENT_DIR` names (`content` unless set), asks Sonnet through `claude` to explain it, and saves the explanation, which quotes the words printed on the figure, as `figure.md`, with an `image.json` that says where it came from. Nothing is cut out of the picture. It is not started while `ANTHROPIC_API_KEY` is set, and the command checks `EMBEDDING_GEMINI_API_KEY` before Sonnet is paid. The command then does what it does for a converted document: one `Document` node, named after the file, and one `Item` node, one point that is embedded from the picture together with its explanation, and the concepts of the explanation.

`--note` is what you know about the picture. It is added to the explanation as a last line, `Note: …`, before the picture is embedded and before concepts are asked for, so the stored text of the item ends with it. A folder takes no note. A picture belongs to no media, so `--category`, `--author` and `--tag` are refused for it, and `--doc-tag` gives it own tags.

The document id is made from the SHA-256 of the picture's bytes, as a document's is made from its PDF, so the same picture under another name is the same document, and the note does not change it. A second run on the same picture asks neither Sonnet nor `claude` a question, because the converted files and the answers are kept. It runs `claude auth status` once, which is free, and Gemini embeds the picture again, as it does for a document. The note is part of the item, so to give a picture another note, delete its document first with `delete-document`: otherwise the mentions of the earlier note stay beside the new ones.

## Labelling a media and a document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- media "Option Volatility and Pricing" --author "Sheldon Natenberg" --tag options --tag volatility
cargo run --release -p rag-ingestion --bin rag-ingest -- media "Hawkes Processes in Finance" --category paper
cargo run --release -p rag-ingestion --bin rag-ingest -- tag <document id> --add greeks --remove volatility
```

A media is a book, a paper or another work. It has a title, a category (`book`, `paper` or `other`), its authors, in order, and free tags, such as "options". Each PDF of a media is one of its documents: a chapter of a book, or the PDF of a paper or another media, which has a title. A document carries a copy of the labels of its media, and it has tags of its own.

The graph keeps one `Media` node for each media, with the properties `title`, `category`, `authors` and `tags`. It has no edge: a document belongs to the media whose title it carries, whatever the capitals. Every `Document` node, and every item point of the document, stores the labels flat, under five keys:

| Key | What it holds |
| --- | --- |
| `media` | The title of the media |
| `category` | `book`, `paper` or `other`: the category of the media |
| `authors` | The authors of the media, as a list, in the order of the media |
| `media_tags` | The tags of the media |
| `tags` | The document's own tags |

A picture that stands alone belongs to no media, so it has only its own `tags`. A label that a document does not have is left out of its points. On the `Document` node, a missing `media` or `category` is left out, and a list with nothing in it stays an empty list. Labels are not embedded and no id is made from them, so a label never changes a document id, an item id or the text that Gemini embeds. A tag is not a concept: nothing in the concept graph changes.

A tag is stored in lower case with no space at either end, and each tag once, so `Options` and ` options ` are one tag. An empty tag or author is refused. An author is stored as it was given, with no space at its ends, and an author given twice is kept once. The title of a media is stored as it was given, with no space at its ends.

An ingest never changes a stored media. When the library has no media of that title, whatever its capitals and the space at its ends, the ingest makes one from `--category`, `--author` and `--tag` (or from the `category`, `authors` and `tags` that an agent gives `ingest_pdf`). When the library has the media, these are not used, and the document takes the labels of the stored media. The ingest writes the labels of the media itself. The own tags of `--doc-tag` are written after the ingest has finished, so a run that stops in the ingest writes none of them, and the same command again finishes the ingest and writes them. `--doc-tag` adds a tag and never takes one away, and an ingest keeps every own tag that the document has.

`media <title>` changes the labels of a stored media: `--category` sets its category, each `--author` gives an author, and each `--tag` gives a tag. A flag that is given replaces that whole label, and a flag that is not given keeps it, so the authors or the tags of a media cannot be emptied from the command line; the media form of the desktop app can empty them. Give at least one flag. The title is matched whatever its capitals and the space at its ends, and it cannot change: it names the media's folder, and it is in the text that was embedded. The command writes the `Media` node first, then each document of the media and every point of it, in both stores, with no embedding and no call of any model, so it costs nothing. The own tags of each document stay as they are. It prints the labels that the media has afterwards and how many documents carry them. A title under which the graph holds no media is refused and changes nothing. A run that stopped half way is finished by running it again.

`tag <document id>` changes the own tags of a stored document, in both stores, with no embedding and no call of any model. Each `--add` adds a tag, and each `--remove` takes a tag away, after the tags to add were added. The labels of its media stay as they are: change them with `media`. It prints the labels that the document has afterwards. The document id is the one that an ingest prints as `document id`, and the one that **Copy id** on the Library tab of the desktop app copies. An id under which the graph holds no document is refused and changes nothing.

## Deleting a document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- delete-document <document id>
```

Removes one document from both stores: its points from the Qdrant collection, and its `Document` node, its `Item` nodes and all their edges from the graph, the `MENTIONS` of its items among them. Other documents are left whole, and so is the `Media` node: a media whose last document is deleted stays in the library with its labels and no document. Concepts, their `RELATES_TO` edges, their points in the concepts collection and their aliases stay, because they belong to no document. The document id is the one that an ingest prints as `document id`, and the one that **Copy id** on the Library tab of the desktop app copies. The command prints how many points and nodes it removed, and refuses an id under which neither store holds anything, so an item id or a mistyped id removes nothing.

Run it again if it stopped half way: it removes what is left. It is also the way to clear a document before it is ingested again after the way it is cut into items has changed, because an ingest never removes the points and nodes of an earlier run.

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

Each result prints the title of its document, its page (the printed page number where there is one), its kind, its label where it has one, its score and its text. A figure also prints the path of its own picture. After the last result come a blank line, the tokens of the embedding of the question, `tokens of gemini-embedding-2: <n> in, 0 out, 0 cache read, 0 cache write (estimated)`, and `cost: under $0.01`. Two notes can follow the score:

- `reached via concept <name>` marks an item that came in through the graph. The name is the first of the concepts, in the order of the steps above, that the item mentions.
- `cited by result <n> as <label>` marks an item that was added in step 6 because result `n` cites it.

The items of step 6 come after the others. They are not counted in the 8 or in the 3 of their document, and they are not in score order with the others. A citation finds only the label as it is printed, in the same document: "Fig. 7-2" does not find "Figure 7-2". `--kind` looks only at items of one kind: `chunk`, `formula`, `figure` or `table`, and any other value is refused. With `--kind` nothing is added in step 6.

`--media "<title>"`, `--author "<name>"`, `--category <category>` and `--tag <tag>` look only at items of documents that have those labels, as "Labelling a media and a document" describes them. Each is optional and `--tag` can be repeated. A document must match everything that is given: the media matches the title of its media whatever the capitals, the author matches any one of its authors whatever the capitals, the category is `book`, `paper` or `other`, and each tag must be a tag of its media or one of its own tags. The filter holds for every item that is printed, the nearest items, the items that the graph adds and the items of step 6, and it works with `--kind` and with `--answer`. When no document matches, the command prints `no items found` and the question is not embedded.

`--answer` asks Sonnet, `claude-sonnet-5-5`, to write an answer from the items that were found. It runs through the `claude` command on your subscription, with no tools, and it is not started while `ANTHROPIC_API_KEY` is set. It makes one call. Before the search, the command checks that the key is not set and runs `claude auth status` once, which is free and asks no question, so nothing is embedded when `claude` cannot answer: it prints `claude cannot write an answer, so nothing was searched for` and the reason. The model is given the question and the items, numbered: the document, the page, the kind, the label, and the text, the raw LaTeX of a formula, or the explanation of a figure with the path of its picture. It replies with claims, and each claim names the numbers of the items that support it. The command prints the answer, not the results, then a blank line, one line for the tokens of each model that the question used (the embedding and Sonnet) and `cost: ≈ $<cost>`. The tokens and the seconds of the `claude` call also go to standard error. After each claim it prints one line for each source: the title of the document (which names the book and the chapter, for a chapter), the printed page, and the kind and label of the item. Under the line of a formula it prints the LaTeX exactly as the document has it, and under the line of a figure it prints the path of the picture. The program writes the citations and the LaTeX, not the model, so a title, a page or a formula is never retyped. A reply with a claim that names no source, or an item that was not given, is an error, and the question is not asked again. When nothing is found, the command prints `no items found` and does not ask the model. When the model replies with no claim, the command prints `the stored items do not answer the question`. The prompt and the JSON Schema of the reply are in `crates/rag-retrieval/src/answer/prompts/`.

## The desktop app

```bash
cargo run --release -p gui
```

The program is `quanty`, built as `target/release/quanty`. It opens one window with three tabs, **Ask**, **Library** and **Ingest**. Ask is one screen of six panels:

| Panel | What it shows |
| --- | --- |
| Ask bar | The question, the mode, and the filters Media, Authors, Tags and Category |
| Answer | The written answer with its citations, and the results by kind: all, formulas, figures, tables |
| Source in Context | The page a citation stands on, with its figures, formulas, tables and concepts |
| Concept Graph | The concepts of the answer and how they link |
| Retrieval Path | The path of the search in five rows, with what each step produced |
| Follow up | Questions to ask next and a box for your own: each is a new search with the same mode and filters |

| Key | What it does |
| --- | --- |
| `⌘1`, `⌘2`, `⌘3` | Go to Ask, Library, Ingest |
| `/` or `⌘K` | Write a question (`Enter` asks it) |
| `J`, `K` | Next and previous result |
| `⌘.` or `Esc` | Stop the search or the answer |
| `⇧⌘S` | Copy the answer with its citations |
| `⌘]`, `⌘[` | Next and previous page of the source |
| `⌘+`, `⌘−`, `⌘0` | Zoom the page while the pointer is over it |

`/`, `J`, `K` and `Esc` do this only while no text box has the keyboard.

It needs what `rag-query --answer` needs: Qdrant and FalkorDB, `EMBEDDING_GEMINI_API_KEY`, and `claude` signed in with `ANTHROPIC_API_KEY` not set. An ask makes one embedding call and one Sonnet call. In the mode **Results only** it makes the embedding call alone. When a service is not ready, the part of the window that needed it says which one and what to do.

Once the answer has landed, the row of tabs of the Answer panel says what the ask used, left of **Share**, such as `12k tokens · ≈ $0.03`. In the mode **Results only** it says what the search used, such as `12 tokens · under $0.01`. In a window too narrow for it beside the tabs, it is left out.

The program reads `.env`, `content/` and `data/` from its home folder. It finds that folder in this order: the folder given with `--home <folder>`; the folder that `QUANTY_HOME` names; the nearest folder at or above the current one that holds a `.env`; the nearest folder at or above the program's own folder that holds one, which is what a start from Finder uses; and last the current folder, with no `.env`.

A citation opens its page when the document's folder is found. The document is looked for under the content folder, by its source file; a folder that is stored with the document is used first, when there is one. A document that is found in neither way shows "The page was not found" with what to do: this is what a document that was ingested from a folder outside the content folder shows.

The filters of the Ask bar are lists. **Media** offers "All media" and each media that has a document. **Authors** offers "All authors" and each author of a document, once. **Tags** lets you tick any number of tags, from the tags of the media and the own tags of the documents. **Category** offers "Any category", Book, Paper and Other. An ask looks only at the documents that match every filter, as the flags of `rag-query` do. The Source panel picks its page by **Media** and **Document**: a chapter shows as "Chapter <number> · <name>", and any other document by its title.

`--fixture <scene>` runs the whole window on built-in data from `samples/content`, with no store, no model and no cost. `--fixture list` prints the scenes: each is one state of the screen, such as `black-scholes` (a full answer), `stores-down` or `first-run`.

The **Library** tab shows each media as a card: its title, a badge for its category (Book, Paper or Other), its authors and its tags. A media that was saved and has no document yet says so. The documents that belong to no media, such as a picture that stands alone, are under "No media". The pencil **✎ Edit** at the right end of a card opens the form of the media in the card: three chips for the category, then **Authors** and **Tags**, each with commas between them. It has no title box, because the title names the media's folder and cannot change. **Save media** is on once something has changed. It writes the labels to the `Media` node, to every document of the media and to every point, the way `rag-ingest media` does, but it replaces every label, so the authors and the tags can also be emptied here. **Cancel** closes the form. A save that is refused keeps the form open and says why.

In the card, each document shows its chapter, as "Chapter <number> · <name>", or its title; its pages and its items; and its own tags. A document whose ingest did not finish says that a search may miss parts of it. **Copy id** copies the document id. **Read** opens the document in the Source panel of Ask. **Edit tags** opens a box for its own tags, with commas between them, and **Save** writes them to both stores the way `rag-ingest tag` does. Every edit is free: no embedding and no model call. No edit can be made while an ingest runs, or while the save of a media is on its way.

The **Ingest** tab is the **Add media** journey, in three steps:

1. The media. The **Media** list holds every stored media, as "<title> · <category>", and last **Add new media…**. A chosen media shows as a card with its title, its category badge, its authors and its tags. Its labels are fixed here, and its pencil **✎ Edit** opens the same form as in the Library. **Add new media…** opens the form of a new media: the category chips, **Media title**, **Authors** and **Tags**, then **Save media** and **Cancel**. A saved media is chosen at once, and it stays in the list each time the app starts, also before its first PDF. A title that the library already has, whatever its capitals, is refused: choose that media from the list.
2. The PDF. **Choose a PDF** opens a file dialog. The PDF of a book takes **Chapter number** and **Chapter name**, which are filled in from a file named `chapter-<number>-<name>.pdf` and else typed. The PDF of a paper or another media takes a **Title**, filled in with the title of the media. **Tags for this PDF** are its own tags; the tags of the media apply as well.
3. The check and the ingest. The check runs by itself once a media and a PDF are chosen and the fields are filled, and again when one of them changes. It does not run on each key: a box is read once it no longer has the keyboard. While it runs, a bar moves. It is free: it counts the pages of the PDF, says how many are converted already, and shows a notice for what would stop a start, such as `claude` not signed in. **Start ingest** is paid work, the same as `rag-ingest pdf`. While the ingest runs, a bar shows the stage: "Reading the PDF", "Converting the pages — page 3 of 12" with the tokens and the cost so far beside it, such as `48k tokens · ≈ $0.42 so far`, "Writing the graph", "Embedding N items", "Storing the items", "Reading the concepts — N of M" and "Linking the concepts — N of M". Keep the app open while it runs. If it stops, start the same PDF again and it carries on. At the end, a notice says what was ingested, the tokens of each model and the cost of the run, and **Add another PDF** clears the PDF and keeps the media chosen.

Not built yet: the notices tray, the help sheet, the health check and the delete of a document. Delete one with `rag-ingest delete-document`.

## The MCP server

`quanty-mcp` lets an AI agent search the stored library, read the pages of its documents, get a cited answer and send a PDF to be ingested, over the Model Context Protocol. How to build and start it, its tools and their costs are in [mcp.md](mcp.md).

## Costs

Every ingest and every question says what it used: the tokens of each model, and about what they cost.

| Where | What it shows |
| --- | --- |
| `rag-ingest <folder>`, `rag-ingest <picture>` and `rag-ingest pdf` | At the end of the summary, one line for each model, `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write`, then `cost of this run: ≈ $<cost>` |
| `rag-ingest pdf` while it runs | `page <n> converted ($<cost>)` for each page |
| `rag-query` and `rag-query --answer` | The same lines after the results or the answer, then `cost: ≈ $<cost>` |
| The desktop app | `48k tokens · ≈ $0.42 so far` beside the running ingest; the tokens of each model and the cost in the notice at its end; `12k tokens · ≈ $0.03` in the row of tabs of the Answer panel |
| The MCP server | `usage` and `cost_usd` in `search`, `answer` and a `done` report of `ingest_status`, as [mcp.md](mcp.md) shows |

A cost is the tokens of each model times the price of that model in the table below, added up. A model that the table does not have is counted at what `claude` reported for it. When a model has neither, the cost is `unknown` (`null` in the MCP server). A cost that would round to nothing shows as `under $0.01`.

These figures are what the API would charge for the tokens. `claude` runs on the subscription that it is signed in to, which bills in its own way, so they show the size of a run and are not a bill. `claude` reports a dollar figure of its own, which is not shown and can differ: for one, it writes some cache entries at the one-hour price, and the table has the five-minute price.

US dollars for one million tokens, as of 9 October 2026:

| Model | Input | Output | Cache read | Cache write |
| --- | --- | --- | --- | --- |
| `claude-fable-5-1` | 10.00 | 50.00 | 0.25 | 12.50 |
| `claude-opus-5-5` | 4.00 | 20.00 | 0.20 | 5.00 |
| `claude-sonnet-5-5` | 2.00 | 10.00 | 0.10 | 2.50 |
| `claude-haiku-5-5` | 0.10 | 0.50 | 0.01 | 0.125 |
| `claude-haiku-4-5` | 1.00 | 5.00 | 0.10 | 1.25 |
| `gemini-embedding-2` | 0.20 | — | — | — |

The Claude prices are from the Claude pricing page (platform.claude.com/docs/en/about-claude/pricing); a cache write is the five-minute write, and Haiku 5.5 is priced for prompts of up to 100,000 tokens. The Gemini price is from the Gemini API pricing page (ai.google.dev/gemini-api/docs/pricing): Gemini Embedding 2, standard paid tier, text input. The names that `claude --model` takes, `haiku`, `sonnet` and `opus`, are Haiku 5.5, Sonnet 5.5 and Opus 5.5, and an id with a date after it, such as `claude-haiku-4-5-20251001`, has the price of the id without the date. The table is the constant `PRICES` in `crates/rag-core/src/usage.rs`.

- The tokens of the embedding are worked out, because the embeddings API gives no count: one token for every four characters of the text that is sent, rounded up, marked `(estimated)`. A picture is counted by its text alone.
- Jev has no price and is left out.
- `ocr` counts the tokens of a page call under the one model that answered, from the whole reply, while a question of the concepts or of an answer counts each model that `claude` names. So a short helper call that `claude` makes with another model inside a page call is priced as the model of the page call.
- A run that fails or is stopped prints no total. The page lines, and the running line of the desktop app, show what it used up to the stop.

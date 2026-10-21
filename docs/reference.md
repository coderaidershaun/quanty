# quanty reference

Every command in full detail. For the short version, see the [README](../README.md).

PDFs are turned into searchable items (text, LaTeX formulas, figures, tables and code listings). An LLM extracts the concepts each item mentions. Concepts are global, so two PDFs that both discuss Black–Scholes attach to the same node, and every new PDF links itself to everything already ingested with no manual work.

## How it works

- **Qdrant** stores every vector (items and concepts).
- **FalkorDB** stores the graph: documents, items and concepts, joined by `HAS_ITEM`, `NEXT`, `MENTIONS` and `RELATES_TO` edges, and one `Media` node for each media, with its title, category, authors and tags. A `Media` node has no edge: a document belongs to the media whose title it carries, whatever the capitals. Every program that connects to FalkorDB makes the graph and its indexes when they are missing: on the `id` of each `Document`, `Item` and `Concept` node, and on the normalised name and the normalised aliases of each concept.
- **Gemini Embedding 2** embeds text, LaTeX and images into one vector space.
- **Claude** (via `claude -p`) extracts and resolves concepts, and writes answers.
- Retrieval finds the closest items by vector search, then follows concepts across documents to pull in related material.

## Crates

| Crate | Purpose |
| --- | --- |
| `ocr` | PDF to text, LaTeX, figures and figure descriptions |
| `rag-core` | Shared types, IDs, config, `Embedder` and `Llm` traits, Qdrant wrapper |
| `model-settings` | The saved model and thinking choices, one each for Ask, Ingest and Datasets, that the desktop app, the command line programs, the MCP server and `ocr` read (`.quanty-model.json`); the file holds `ask`, `ingest` and `datasets`, each with `provider`, `model` and `thinking`, and a file saved by an older version of quanty, with one choice, gives that choice to all three |
| `graph` | FalkorDB graph store |
| `rag-ingestion` | Chunking, embedding, concept extraction and resolution, and the insights of a chapter (`rag-ingest`) |
| `rag-retrieval` | Vector search, graph expansion, ranking, answers with citations (`rag-query`) |
| `rag-dataset` | Builds Gemma 4 training data from the converted documents: whole sections, synthetic texts, checked fine-tuning examples and a manifest (`rag-dataset`) |
| `gui` | The desktop app: a question, its answer with citations, the page each one stands on, the concept graph and the path of the search (`quanty`) |
| `mcp` | An MCP server, so that an AI agent can search, read pages, get answers, ingest a PDF and build a training dataset (`quanty-mcp`) |

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

`ocr` converts the PDF of a book's chapter. The PDF of a paper or another media, and a part of a book that is not a chapter, such as its index, are converted by `rag-ingest pdf` (see "Ingesting a PDF"). The file must be named `chapter-<number>-<name>.pdf`, and `--book` is the title of the book. Every page is broken into its pieces (headings, text, formulas, figures, tables, code listings and footnotes) and saved under `content/<media folder>/chapter-<number>/page-num-<i>/`, with a `page.json` for each page and a `chapter.json` for the document. A code listing is saved as printed, with its line breaks and indentation, in `NN-code.txt`, and `page.json` keeps its label, caption, summary and its language in lower case, such as `python`, or none when it cannot be told. Each figure also gets its own picture, `NN-figure.png`, cut out of the page beside its `.md` file, and a figure that cannot be cut out keeps the whole page as its picture and is listed in the summary. `--out <folder>` saves somewhere other than `content`. The command only saves files: nothing is put into Qdrant or FalkorDB.

The command reads `CONVERTER_JEV_API_KEY` from `.env`. The model is the Ingest choice of model and thinking. When that choice is a Claude model, the command refuses to run while `ANTHROPIC_API_KEY` is set, so the work is billed to the `claude` subscription. A second run on a finished chapter makes no outside call and costs nothing. An interrupted run picks up at the pages that are missing. A different PDF for the same chapter is refused. A run holds the file `conversion.lock` in the document's folder until it ends, so a second run on the same document at the same time, from any program, is refused with `another run is converting <folder> at this moment; wait for it to finish, then run again`. Only a run that is going holds the lock, so a run that was killed leaves nothing locked.

Jev is asked again after a pause only when its request fails on the network, on its server or at its rate limit. A missing or refused key, or a request that Jev calls bad, is not asked again. A page whose math check still fails goes to Sonnet.

The summary at the end counts the pages, the routes, the pieces and the paid calls of this run, and gives one line for each model that answered: `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write`. It gives no cost, because `ocr` has no table of prices: `rag-ingest pdf` gives the cost of a conversion. See "Costs".

Every way of converting a PDF uses the same folders. The media folder is the title of the media in lower case, with one dash for each run of characters that are not letters or digits, so "Option Volatility and Pricing" is `option-volatility-and-pricing`. In it, each document has a folder of its own:

| Document | Folder |
| --- | --- |
| A chapter of a book | `<content>/<media folder>/chapter-<number>/` |
| The PDF of a paper or another media | `<content>/<media folder>/<document title folder>/`, the title of the document made into a folder name the same way |
| A part of a book that is not a chapter | `<content>/<media folder>/<part title folder>/`, the title of the part made into a folder name the same way |

The folder of a document with a title of its own comes from that title without the media's title and the comma in front of it, so the index of "Option Volatility and Pricing", which is stored as "Option Volatility and Pricing, Index", is in `option-volatility-and-pricing/index/`.

`chapter.json` says what the folder holds: `media-title`, `name` (`{"chapter": {"number": …, "name": …}}` for a chapter, `{"title": …}` for a document with a title of its own, such as a paper or a part of a book that is not a chapter), `source-file`, `source-sha256` (the SHA-256 of the PDF), `page-count` and `finished`.

An ingest adds one more file to the folder, `insights.json`, which the conversion never writes and a conversion that goes on never removes. It holds what the model wrote about the whole document: `model` (as its agent names it), `prompt-version` (`1`), `insights` (a list; each one has `text`, a short Markdown paragraph that may hold `$…$` or `$$…$$`, and `pages`, the positions from 1 of the pages it rests on, counted the way a stored item counts its `page`), `takeaways` (a list of one-line Markdown points) and `cheat-sheet` (Markdown with short `###` sections, formulas and small tables). Only a chapter, and a document of a media that is not a book such as a paper, gets the file: a part of a book that is not a chapter (a document named by a title, in a media of the category `book`, such as its index, table of contents, front cover, info, glossary, preface or supplementals) gets none, and `ocr::gets_insights` tells which kind a document is. A file that is there is kept until it is removed, so converting the pages again does not write it again; deleting the document removes its folder, and the file with it. `ocr::ChapterInsights::read` gives `None` for a folder that has no such file, and a typed error for one that cannot be read.

Later crates read a converted document back, in reading order, with `ocr::read_chapter`.

## Checking the stores

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

Prints one line each for Qdrant, FalkorDB and the sign-in of the Ingest choice's agent, and exits with 1 when any of them is not ready. The addresses come from `QDRANT_URL` (the gRPC port, default `http://localhost:6334`) and `FALKORDB_URL` (default `falkor://localhost:6379`). The environment wins over `.env`.

If another program on your machine, such as a Homebrew Redis, already listens on port 6379, `localhost:6379` reaches that program and not the FalkorDB container. The FalkorDB line then fails with "did not answer like FalkorDB". Stop the other program or map the container to another port, and set `FALKORDB_URL` to match.

## Ingesting a converted document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- samples/content/quanty-sample-notes/chapter-2
```

Reads one converted document folder, of a book's chapter or of any other document, with `ocr::read_chapter` and stores its items in a Qdrant collection, `items` unless `QDRANT_ITEMS_COLLECTION` names another. Text becomes chunks of about 300 to 500 tokens that never run past a heading, and a paragraph that a page break cut in two is joined again. Each formula, figure, table and code listing is an item of its own. A code listing that page breaks cut into parts, one on each page, is joined again into one item, whole: a code piece continues the listing before it when it is on the next page, nothing but a footnote comes between them, it has no label and no caption of its own, and its language is the same or one of the two is not known. A code item keeps the code as printed, with its language, and it is embedded as its label and caption, its language, a one-sentence summary and the code. A figure is embedded as its own picture together with its explanation, as one vector, and its payload keeps the path of the picture. Every item carries the title of its document and the section it sits in.

A formula, a figure, a table or a code listing that has a printed label, such as "(7.3)", "Figure 13-4", "Table 1-1" or "Listing 2.1", stores it in the payload as `label`. A code item also stores its language in lower case as `language`, when it is known. A chunk stores the labels of the figures, tables and equations that its text points at as `cites`, each once, in reading order. A footnote marker is not one of them. A code listing that page breaks cut into parts also stores the place and the printed number of the page it ends on as `last_page` and `last_printed_page`, while `page` and `printed_page` are those of the page it starts on. Each of these fields is left out of the payload when there is nothing to store. A document that was stored before `label` and `cites` existed does not have them, so ingest it again to add them. In the same way, a document stored before `last_page` existed keeps its cut listings, one item for each page, until it is ingested again.

The ids are computed from the document's source hash and the place of the item in the document. Running the command again on the same document first removes what the earlier run stored of it: its points, its `Item` nodes with their edges, the `MENTIONS` of its items, and its mark of being ingested whole. So a document that is now cut into other items, or whose items now name other concepts, keeps nothing of the earlier run. Its own tags stay, and so do the concepts. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each run by the token.

The command also writes the document to the FalkorDB graph, `quanty` unless `FALKORDB_GRAPH` names another: one `Document` node, one `Item` node for each item, an edge `HAS_ITEM` from the document to each item, and an edge `NEXT` from each item to the one after it in reading order. An `Item` node has the same id as its point in Qdrant. The collection is prepared and the graph is written before anything is embedded, so a store that is down fails the run before Gemini bills anything, and a run that stopped half way is finished by running it again. A second run on the same document replaces the nodes and edges of the first, so it adds none. The `Document` node also has the property `ingested_items`, the number of its items, which is the mark that the document is ingested whole. An ingest takes the mark away when it starts and sets it as its last step, only when no item was skipped. Only `rag-ingest pdf` reads it, as "Ingesting a PDF" describes.

The media of the document is the one that `chapter.json` names. `--category` (`book`, the default, `paper` or `other`), `--author`, given once for each author, and `--tag`, given once for each tag, label that media. They are used only when the library does not have the media yet, because an ingest never changes a stored media: see "Labelling a media and a document". `--doc-tag`, given once for each tag, adds own tags to the document after the ingest has finished.

After the points are stored, and before the concepts are read, the command asks the agent of the Ingest choice for the insights, the takeaways and the cheat sheet of the whole document, in one question, and saves them as `insights.json` in the converted folder. The question holds the document as it is stored: the first line names it, a line `[page <n>]`, or `[page <n>, printed <p>]`, starts each page, a line `Section: <headings>` names the section, and the items follow in reading order, a formula between `$$` and a code listing fenced. The prompt and the JSON Schema of the answer are in `crates/rag-ingestion/src/ingest/insights/prompts/`. A reply is accepted as a whole: it needs at least one insight, at least one takeaway and a cheat sheet that is not blank; each text is trimmed, and a page number of an insight that no `[page <n>]` line names is dropped. A question that fails, and a reply that is not accepted, are tried once more. An error that would fail every question, such as a usage limit or a sign-in that is lost, ends the step at once. When the tries fail nothing is saved, and that is not an error of the ingest: the items stay stored, the document is marked ingested whole, and the summary says `insights: not written: <reason>; the items are stored, so run `rag-ingest insights --document <id>` to write them`. The next run of the same PDF, or that command, writes only the file. A document whose text is more than about 100,000 tokens (four characters to a token) is not asked about, and the summary says so in the same way. A file that is there and can be read is not asked for again: the summary says `insights: already saved in <file>, so nothing was asked`. A file that cannot be read counts as none and is written over. A picture that stands alone gets no insights, and neither does a part of a book that is not a chapter, such as its index: the agent is not asked, no file is written, the step is not told, and the summary says `insights: none, because a part of a book that is not a chapter gets none`. There is no flag that changes this. `claude` is given up to five minutes for this answer, which is long, instead of the two minutes of each of its other questions; Codex is given five minutes for every question. The question is paid work like the others, and its tokens are counted in the summary.

Last, after the points are stored, the command asks `claude` which concepts each item discusses and how they relate, four items at a time. Every item is asked about, a figure as its explanation with no picture. An item that is alone in its document, such as a picture, is asked about together with the five stored items nearest to it from other documents, under the heading "Possibly related material", so that the model names its concepts as they are named elsewhere. The model is told to name only what the item itself shows. The kept answer does not depend on that material, so an item is asked about once. `claude` runs on your subscription with the model and thinking of the Ingest choice, with no tools and in safe mode. It is not started while `ANTHROPIC_API_KEY` is set, so that the work is not billed to the API: the run stops before anything is embedded or stored, with a message that says to unset the key. The prompt and the JSON Schema of the answer are in `crates/rag-ingestion/src/ingest/concepts/prompts/`.

The answers are written to the graph. A `Concept` node has an id, a name, a one-line definition and a list of aliases, and it belongs to no document. An edge `MENTIONS` goes from an item to each concept it discusses, with the wording the item used. An edge `RELATES_TO` goes from one concept to another, with one of the types `DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF` and `USED_FOR`, and the item that stated it. A concept named by two items is one node with two `MENTIONS`. A relation is dropped and counted when it names a concept that neither its own answer nor the graph holds, or when it joins a concept to itself. A mention or a relation whose item or concept is missing from the graph is not written, and the run stops with an error that says how many of them were written. The others are written.

A name is matched to a stored concept in four steps, one concept at a time, so that a name an earlier item made is seen by a later one:

1. The name is compared with the name and the aliases of every stored concept in its normalised form: lower case, with every run of punctuation or hyphens turned into one space. So "Black–Scholes model" and "black-scholes model" are one concept. A match is linked and nothing else is written.
2. When nothing matches, the name and its definition are embedded as "name: definition", and the nearest stored concept is looked up in a second Qdrant collection. It is `concepts` unless `QDRANT_CONCEPTS_COLLECTION` names another, and it holds one point for each concept, under the id of the concept's node, with the name and the aliases as its payload.
3. A cosine score of 0.95 or more links the name to that concept and adds the name to its aliases, in the node and in the point. A score under 0.75 makes a new concept, written to the graph and to the collection. Between the two, `claude` is asked whether the two concepts are the same, with both names and both definitions: yes links and adds the alias, no makes a new concept. Only the nearest concept is compared, and the two thresholds are the constants `LINK_SCORE` and `ASK_SCORE` of `crates/rag-ingestion/src/ingest/concepts/resolve.rs`.
4. The `MENTIONS` edge is written with the wording the item used. When two names of one answer end as one concept, the item mentions it once, with the first wording.

Every decision is added to a log, `data/concept-decisions.jsonl` unless `CONCEPT_DECISION_LOG` names another file, as one line of JSON. A line has `item` (the id of the item that named the concept), `name` (as the item wrote it) and `rule`: `exact-name`, `high-score`, `llm-same`, `llm-different`, `low-score` or `nothing-stored`. Where they apply it also has `matched` (the stored concept the name was linked to, with its `id` and `name`), `nearest` (the nearest stored concept, which was turned down), `score` and `created` (the id of the new concept). A line is added after the writes it led to have succeeded.

If `claude` fails twice to say whether two concepts are the same, the run stops with a message that names both and says to run the same command again. Nothing is guessed: a guess of "different" would make a second concept that nothing can merge, and a guess of "yes" would make a wrong alias that every later item matches. The items are stored and what was linked so far stays, and the next run goes on from there. A usage limit stops the run in the same way.

Every good answer is kept as a file in the folder that `CONCEPT_CACHE_DIR` names, `data/concept-cache` unless set. The name of the file is made from the prompt, its version, the schema, the model and the text that was sent, so a second run on the same document asks `claude` no question and adds no node, no edge, no concept point and no alias: every name is now found in step 1, so no concept is embedded and no comparison is asked. The answer to a question about two concepts is kept in the same folder, and the summary counts these questions with the others. Every run first checks that `ANTHROPIC_API_KEY` is not set and runs `claude auth status` once, which is free and asks no question. A set key, a missing sign-in, or a `claude` program that cannot be started stops the run there, with `claude cannot be asked for the concepts of the document, so nothing was embedded or stored` and the reason. A usage limit, or a sign-in that is lost during the run, stops the run with a clear message and is never tried again: the document is stored and can be searched, and running the same command again goes on from the answers that are kept. Any other failure of one item is tried once more, and then the item is skipped, not kept, and named in the summary, and the run goes on. The next run asks about it again.

After the lines about items and points, the summary prints how many concepts were created and how many were linked to an existing one, the mentions written, the relations written and dropped, the `claude` calls made and the cache hits, and the number of items skipped with one line for each, and the `insights:` line. Last come one line for each model that the run used, `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write`, with ` (estimated)` after the embedding, whose count is worked out, and `cost of this run: ≈ $<cost>`. A kept answer costs nothing. For a picture, the Sonnet call that explained it is one of the lines. See "Costs".

## Ingesting a PDF

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf --book "Option Volatility and Pricing" samples/chapter-1-sample-pages.pdf
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf --paper "Hawkes Processes in Finance" --author "First Author" --author "Second Author" --tag hawkes --doc-tag survey hawkes-notes.pdf
```

Does in one run what "Converting a book's chapter" and "Ingesting a converted document" do in two, with no one in between, for the PDF of any media.

| Flag | What it gives |
| --- | --- |
| `--book <title>`, `--paper <title>` or `--other <title>` | The title of the media, and its category. Give exactly one of the three |
| `--title <title>` | The title of the document. For a paper or another media, without it the document takes the title of the media. For a book, it names a part that is not a chapter, such as its index, and the PDF can have any name; without it the PDF of a book is a chapter |
| `--author <name>` | An author of the media. Give it once for each author, in order |
| `--tag <tag>` | A tag of the media. Give it once for each tag |
| `--doc-tag <tag>` | A tag of this document only. Give it once for each tag |

`--author` and `--tag` are used only when the library does not have the media yet, because an ingest never changes a stored media. The category of the flag is also used only to make a new media. When the library has a media of that title, whatever its capitals and the space at its ends, the stored category wins: it says how the PDF is named and which category the document carries. When it is not the category of the flag, the command says so in one line on standard error: `the library has "<title>" in the category <category>, so this pdf is named and labelled by that category, not by --<flag>`. To change the category of a stored media, use `rag-ingest media`.

The PDF of a book is a chapter when it has no `--title`, and then it must be named `chapter-<number>-<name>.pdf`. The number and the name of the chapter come from the file name, with a capital letter for each word of the name, and the title of the document is "<book>, chapter <number>: <name>", such as "Option Volatility and Pricing, chapter 1: Sample Pages". With `--title`, the PDF of a book is a part of the book that is not a chapter, such as its index or its glossary, and it can have any name: the title of its document is "<book>, <title>", such as "Option Volatility and Pricing, Index". The Library lists such a part by its own title, "Index", after the chapters of the book. The PDF of a paper or another media can have any name, and the title of its document is `--title`, or else the title of the media. A missing flag, a path that is not there and a book's PDF with another name and no `--title` are each refused with a message before the first page is converted, and cost nothing. A book's PDF with another name and no `--title` is refused with `rename it to chapter-<number>-<name>.pdf, or give --title when it is a part of the book that is not a chapter` and the hint that a media the library does not have yet can take `--paper` or `--other`; when the library has the media as a book and the flag is `--paper` or `--other`, with `the library has "<title>" in the category book, so its pdf must be named chapter-<number>-<name>.pdf; rename <pdf>, or give --title when it is a part of the book that is not a chapter, such as its index`.

The PDF is converted by `ocr` into the folder that `CONTENT_DIR` names (`content` unless set), in the folders of "Converting a book's chapter": `<media folder>/chapter-<number>/` for a book's chapter, and `<media folder>/<document title folder>/` for any other document, which for a part of a book is `<media folder>/<part title folder>/`. The converted folder is then ingested with the steps of "Ingesting a converted document", and the own tags of `--doc-tag` are written last.

The command needs `CONVERTER_JEV_API_KEY` for the conversion and `EMBEDDING_GEMINI_API_KEY` for the embedding, each from the environment or from `.env`. A conversion is not started while `ANTHROPIC_API_KEY` is set, so that the work is billed to the `claude` subscription and not to the API. The embedder is set up and both stores are checked before the first page is converted, so a missing Gemini key or a store that is down stops the run before any page is paid for. The graph is read before the PDF is named, because a stored media's category says how the PDF is named.

While it runs, the command prints one line on standard error for each step, so that standard output holds only the summary at the end:

| Line | When |
| --- | --- |
| `checking what is already stored` | The stores are asked whether the PDF is ingested already |
| `opening the pdf` | The conversion starts: it reads what is saved, tries the Jev key and counts the pages |
| `converting <n> of <total> pages (<done> already done)` | The pages are counted, and `<n>` of them are left to convert |
| `page <n> cut out` | A page is cut out of the PDF, which costs nothing. Every page that is left is cut out before the first one is converted |
| `page <n> converted ($<cost>)` | A page is converted. The cost is its calls at the prices of "Costs", or `page <n> converted (cost unknown)` when a model of the page has no price |
| `page <n> failed` | A page failed |
| `writing the graph` | The document and its items are written to the graph |
| `embedding <n> items` | Gemini embeds the items |
| `storing the items` | The points are written to Qdrant |
| `writing the insights, takeaways and cheat sheet` | The agent is asked for the insights, the takeaways and the cheat sheet of the chapter. It is not told when the chapter's `insights.json` is saved already, nor for a part of a book that is not a chapter |
| `reading the concepts of <n> items` | `claude` is asked which concepts the items discuss |
| `linking the concepts of <n> items` | The concepts are matched to the stored ones and linked |

A PDF that is already ingested prints only the first of these lines, and `writing the insights, takeaways and cheat sheet` when it gets insights and its `insights.json` is not saved yet. The tokens and the seconds of each `claude` question of the concepts are also written to standard error.

The summary is the summary that `ocr` prints without its lines of tokens (the pages processed, with how many were converted now and how many were already done, the routes, the pieces, the calls and the pages to check), then the lines that an ingest of a converted folder prints, which count the items written by kind (chunks, formulas, figures, tables and code), the concepts created and the concepts linked to existing ones. Then come the tokens of each model of the whole run, the conversion and the ingest together, so a model that both used has one line, under its name in the table of "Costs". One line closes it: `cost of this run: ≈ $<cost>`, for the conversion and the ingest together.

A page that fails stops the run, and the message names its page number. The pages that were converted stay saved, so the same command goes on from the pages that are missing and then ingests the document. A run that stops during the ingest, for example at a usage limit of `claude`, is finished the same way: the same command converts nothing more and goes on from the answers that are kept.

A second run on the same PDF prints `already ingested` with the document id and the number of items. It makes no `ocr`, embedding or `claude` call and writes nothing to either store, except the own tags of `--doc-tag`, and except that when the chapter's `insights.json` is not saved yet it asks the agent for the insights alone and writes that file: it then prints `already ingested: both stores hold all <n> items of this document, so nothing was converted, embedded or stored`, the `insights:` line, the tokens of that question and its `cost of this run`. For a part of a book that is not a chapter, and for a chapter whose file is saved, it prints the usual `already ingested` line and then its `insights:` line, and asks nothing. The document id is made from the SHA-256 of the PDF, so the PDF is the same document under any file name. The command takes a document as ingested when both stores agree: the `Document` node in the graph has the mark `ingested_items` with a number `n`, and the Qdrant collection holds exactly `n` points of the document. The count of points is what stops the mark from being believed after a collection was emptied or changed, or after a `delete-document` that stopped half way.

The mark is set by every ingest, of a converted folder, of a picture or by `pdf`, and only as its last step, so it is never there for a run that stopped half way. An ingest in which an item was skipped does not set it either, so the next run goes on from there. This also means that a document with an item that fails on every run is never marked: each `pdf` run on it converts nothing again, but ingests it again, and embeds it again.

`rag-ingest <folder>` never reads the mark. It always embeds and stores again, so it is the way to ingest a document that is already stored once more.

## Ingesting a lone picture

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- chart.png --note "A chart from a book on option trading."
```

A picture that stands alone, such as a chart, is ingested as a document of one figure. The file must be a PNG or a JPEG. `ocr` copies it to `images/<the first 16 hex digits of its SHA-256>/` under the folder that `CONTENT_DIR` names (`content` unless set), asks Sonnet through `claude` to explain it, and saves the explanation, which quotes the words printed on the figure, as `figure.md`, with an `image.json` that says where it came from. Nothing is cut out of the picture. It is not started while `ANTHROPIC_API_KEY` is set, and the command checks `EMBEDDING_GEMINI_API_KEY` before Sonnet is paid. The command then does what it does for a converted document: one `Document` node, named after the file, and one `Item` node, one point that is embedded from the picture together with its explanation, and the concepts of the explanation.

`--note` is what you know about the picture. It is added to the explanation as a last line, `Note: …`, before the picture is embedded and before concepts are asked for, so the stored text of the item ends with it. A folder takes no note. A picture belongs to no media, so `--category`, `--author` and `--tag` are refused for it, and `--doc-tag` gives it own tags.

The document id is made from the SHA-256 of the picture's bytes, as a document's is made from its PDF, so the same picture under another name is the same document, and the note does not change it. A second run on the same picture asks neither Sonnet nor `claude` a question, because the converted files and the answers are kept. It runs `claude auth status` once, which is free, and Gemini embeds the picture again, as it does for a document. The note is part of the item, so a picture with another note has its concepts asked for again, and the mentions of the earlier note are removed.

## Writing the insights of stored chapters

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- insights --document 5f3c2a1e-9b04-5d6e-8a17-2c4b7e90f1d3
cargo run --release -p rag-ingestion --bin rag-ingest -- insights --media "Option Volatility and Pricing" --dry-run
cargo run --release -p rag-ingestion --bin rag-ingest -- insights --all
```

Writes the missing `insights.json` of chapters that are stored already, from their converted text, as "Ingesting a converted document" describes. Use it for chapters that were ingested before insights were written, and for chapters whose insights could not be written. A part of a book that is not a chapter gets none, so the command skips it, whichever of the three flags chose it, and says so. It embeds nothing and writes to neither store: it reads the graph and the content folder only, so it needs no Gemini key and no Qdrant.

| Flag | What it chooses |
| --- | --- |
| `--document <id>` | The one document with this document id |
| `--media <title>` | Every stored document of the media with this title, whatever its capitals |
| `--all` | Every stored document that belongs to a media. A picture that stands alone belongs to none |
| `--dry-run` | Lists what would be written and asks no model, so it costs nothing |

Give exactly one of the first three. A document id that the graph does not hold, and a media with no document, are refused with a message and change nothing. The folder of each document is found by listing the content folder for a finished converted chapter with the same source hash, so a document whose folder is not there has no converted chapter.

The command prints one line for each chosen document, in the order of their titles, as `<title> (<document id>): <what>`. With `--dry-run` the line says `would write <folder>/insights.json`, `insights already saved`, `no converted chapter under <content folder>` or, for a part of a book that is not a chapter, `insights: none, because a part of a book that is not a chapter gets none`, and the last line is `dry run: <a> to write, <b> already saved, <c> without a converted chapter, <d> parts of a book with none; no model was asked`. Without it, the line is the `insights:` line that an ingest prints (`insights: written to <file>`, or `insights: not written: <reason>; …`), `insights already saved`, the line of a missing chapter or the line of a part of a book that gets none, and the last lines are `written: <a>, already saved: <b>, not written: <c>, parts of a book with none: <d>`, the tokens of each model and `cost of this run: ≈ $<cost>`. A chapter is asked about one after another, each in one question that is tried twice, as in an ingest.

The agent of the Ingest choice is checked once, which asks no question, before the first chapter. A set `ANTHROPIC_API_KEY`, a sign-in that is missing or a `claude` program that cannot be started ends the command with a message before anything is asked, and a run with nothing to write starts no model at all, such as a run that chose only the parts of a book. The exit code is 0 when every chosen document that gets insights has its file at the end, and always for a dry run, and 1 otherwise: a chapter whose insights could not be written, a document with no converted chapter, or an error. A chapter whose question fails is printed as not written, with the reason, and the command goes on to the next one. It also goes on after a usage limit or a lost sign-in, so each chapter that is left then fails at once with the same reason.

## Labelling a media and a document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- media "Option Volatility and Pricing" --author "Sheldon Natenberg" --tag options --tag volatility
cargo run --release -p rag-ingestion --bin rag-ingest -- media "Hawkes Processes in Finance" --category paper
cargo run --release -p rag-ingestion --bin rag-ingest -- media "Hawkes Processes in Finance" --no-tags
cargo run --release -p rag-ingestion --bin rag-ingest -- tag <document id> --add greeks --remove volatility
```

A media is a book, a paper or another work. It has a title, a category (`book`, `paper` or `other`), its authors, in order, and free tags, such as "options". Each PDF of a media is one of its documents: a chapter of a book, a part of a book that is not a chapter, such as its index, or the PDF of a paper or another media, which has a title. A document carries a copy of the labels of its media, and it has tags of its own.

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

`media <title>` changes the labels of a stored media: `--category` sets its category, each `--author` gives an author, and each `--tag` gives a tag. A flag that is given replaces that whole label, and a flag that is not given keeps it. `--no-authors` takes every author away and `--no-tags` every tag; neither can be given with `--author` or `--tag`. Give at least one flag. The title is matched whatever its capitals and the space at its ends, and it cannot change: it names the media's folder, and it is in the text that was embedded. The command writes the `Media` node first, then each document of the media and every point of it, in both stores, with no embedding and no call of any model, so it costs nothing. The own tags of each document stay as they are. It prints the labels that the media has afterwards and how many documents carry them. A title under which the graph holds no media is refused and changes nothing. A run that stopped half way is finished by running it again.

`tag <document id>` changes the own tags of a stored document, in both stores, with no embedding and no call of any model. Each `--add` adds a tag, and each `--remove` takes a tag away, after the tags to add were added. The labels of its media stay as they are: change them with `media`. It prints the labels that the document has afterwards. The document id is the one that an ingest prints as `document id`, and the one that **Copy id** on the Library tab of the desktop app copies. An id under which the graph holds no document is refused and changes nothing.

## Deleting a document

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- delete-document <document id>
```

Removes everything that quanty holds of one document:

- its points from the Qdrant collection;
- its `Document` node, its `Item` nodes and all their edges from the graph, the `MENTIONS` of its items among them;
- its converted folder, with its pages and pictures, from the folder that `CONTENT_DIR` names (`content` unless set);
- the copy of its PDF under `<CONTENT_DIR>/_uploads`, when the MCP server saved one. The copy goes only when it is this document's PDF: it has the file name of the document's `source-file`, it is in the folder of the media, and its SHA-256 is the one in the document's `chapter.json`. A file with the same name and other bytes stays.

A picture that stands alone goes with its folder under `images/`.

What it never removes:

- a PDF outside the content folder, such as the one given to `rag-ingest pdf` or picked in the desktop app;
- a converted folder that is not under the folder that `CONTENT_DIR` names. The command finds the folders of a document by listing that folder, and never by a path that a store names;
- a converted folder whose `chapter.json` cannot be read, because it is not provably this document's;
- other documents, which stay whole;
- the `Media` node and the folder of the media: a media whose last document is deleted stays in the library with its labels and no document;
- concepts, their `RELATES_TO` edges, their points in the concepts collection and their aliases, because they belong to no document;
- the answers that are kept about concepts, and the log of concept decisions.

The converted pages go with the folder, so to ingest the same PDF again converts it again, which is paid work: see [Costs](#costs). The command itself costs nothing.

The document id is the one that an ingest prints as `document id`, and the one that **Copy id** on the Library tab of the desktop app copies. The command asks no question. It prints the document id, how many points and nodes it removed, each converted folder it removed and each uploaded PDF it removed, or `none` when there was no such folder or file. It refuses an id under which neither store holds anything and the content folder holds no converted folder, so an item id or a mistyped id removes nothing.

A document that is being converted at this moment, by `ocr`, `rag-ingest pdf`, the desktop app or the MCP server, is refused, and nothing of it is removed: the message says `the document is being converted in <folder> at this moment`. Run the command again when that conversion has ended. Only the conversion is checked: an ingest that has converted its pages and still stores the document is not seen, so wait for that ingest to end before you delete the document.

The steps run in this order: the content folder is listed and each converted folder of the document is taken, the points go, the upload copy and then the converted folder go, and the nodes go last. So while anything of the document is left, the graph still lists it under its media. Run the command again if it stopped half way: it removes what is left, also when only the converted folder is left. One case is not found again: if the command is stopped while it removes a converted folder, a part of that folder can stay without its `chapter.json` (`image.json` for a picture). No delete finds it again, so remove what is left of that folder by hand before the same PDF is ingested again.

## Deleting a media

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- delete-media "<title>"
```

Removes a whole media by its title. The title is matched whatever its capitals and the space at its ends.

1. Every document that carries the title goes, in the order of their ids, as `delete-document` removes it: points, nodes, converted folder and uploaded PDF.
2. The folder of the media under the content folder and its folder under `_uploads` go, each only when nothing is left in it. Two titles that differ only in punctuation, such as "Quanty Sample Notes" and "Quanty Sample-Notes", name one folder, so a folder that still holds a document of the other title stays. A hidden file, such as the `.DS_Store` that Finder makes, counts as something left, so the media folder then stays. A converted folder of the media that no stored document names is not found, and it stays, and then the media folder stays too.
3. The `Media` node goes last.

It works for a media that has no document yet, such as one saved before its first PDF, and for documents whose title no `Media` node carries. A title that no media and no document carries is refused, and nothing is removed. Everything that `delete-document` leaves stays here too: concepts, the answers that are kept, the log of concept decisions, and every media and document of another title.

The command asks no question and costs nothing, but the converted pages go, so to ingest a PDF of the media again converts it again, which is paid work. It prints the title as it was given, how many documents it removed and the id of each, the points and the nodes removed from all of them, each converted folder and each uploaded PDF it removed, each media folder it removed, and whether the `Media` node was removed (`media removed from the graph: no` when documents carried a title that no media has). Each list of folders or PDFs says `none` when it is empty.

A document that is being converted stops the command at that document, as it stops `delete-document`. The documents before it are gone, and the same command again goes on from there. This holds for any error: run the command again if it stopped half way. The exception of `delete-document` holds too: if the command is stopped while it removes a converted folder, a part of that folder can stay without its `chapter.json`, and you remove what is left of it by hand.

## Asking a question

```bash
cargo run --release -p rag-retrieval --bin rag-query -- "What is the Black–Scholes partial differential equation?"
cargo run --release -p rag-retrieval --bin rag-query -- --answer "What is the Black–Scholes partial differential equation?"
```

The command reads what an ingest wrote: the items collection that `QDRANT_ITEMS_COLLECTION` names (`items` unless set), the concepts collection that `QDRANT_CONCEPTS_COLLECTION` names (`concepts` unless set) and the graph that `FALKORDB_GRAPH` names (`quanty` unless set). Every ingest creates both collections, also when it finds no concept. The command needs `EMBEDDING_GEMINI_API_KEY`, and Gemini bills each question by the token. A store that is down stops the command with a message that names it. FalkorDB is connected to before the question is embedded, so a graph that is down costs nothing.

The search has six steps, in this order:

1. The question is embedded, and the 8 items nearest to it are the seeds.
2. The search starts from the 3 concepts nearest to the question and the seed concepts, which are the concepts that the seeds mention.
3. The graph adds the items that mention one of the concepts of step 2, and the concepts that are one `RELATES_TO` edge from one of them, with the items that mention those. The graph adds at most 50 items.
4. The seeds and the added items are ranked together by one more query to Qdrant that looks only at those items, so they are all scored by the same cosine to the question.
5. No document gives more than 3 results, and the first 8 results that are left are kept.
6. When no `--kind` is given, each figure, table and equation that a result cites by its printed label, such as "Figure 13-4", "Table 7-2" or "(7.3)", is added if it is in the same document and is not in the results yet.

The four numbers of these steps are constants in `crates/rag-retrieval/src/search/mod.rs`. The two that matter most are `RESULTS_PER_QUERY` (8) and `MAX_RESULTS_PER_DOCUMENT` (3). All four are starting values. A document gives at most three results in steps 1 to 5, also when it is the only document in the store. An item that is not a seed never scores above a seed, so an item that came in through the graph is shown only where the cap left a place free. When no document has more than three of the eight nearest items, no result comes in through the graph. With no concept in the graph, the results are the nearest items only.

Each result prints the title of its document, its page (the printed page number where there is one; a code listing that spans pages prints its first and last page, such as `155 to 159`), its kind, its label where it has one, its language where it is code with a known language, its score and its text. A figure also prints the path of its own picture. After the last result come a blank line, the tokens of the embedding of the question, `tokens of gemini-embedding-2: <n> in, 0 out, 0 cache read, 0 cache write (estimated)`, and `cost: under $0.01`. Two notes can follow the score:

- `reached via concept <name>` marks an item that came in through the graph. The name is the first of the concepts, in the order of the steps above, that the item mentions.
- `cited by result <n> as <label>` marks an item that was added in step 6 because result `n` cites it.

The items of step 6 come after the others. They are not counted in the 8 or in the 3 of their document, and they are not in score order with the others. A citation finds only the label as it is printed, in the same document: "Fig. 7-2" does not find "Figure 7-2". `--kind` looks only at items of one kind: `chunk`, `formula`, `figure`, `table` or `code`, and any other value is refused. With `--kind` nothing is added in step 6.

`--document "<title or id>"` looks only at items of one document. Give its whole stored title, whatever the capitals and the space at its ends, or its document id, which is the one that an ingest prints as `document id` and that **Copy id** on the Library tab of the desktop app copies. The stored title of a chapter is "<book>, chapter <number>: <name>", of a part of a book "<book>, <part>", and of a paper or another work its own title. A part of a title does not match: "Index" does not find "Option Volatility and Pricing, Index". `--media "<title>"`, `--author "<name>"`, `--category <category>` and `--tag <tag>` look only at items of documents that have those labels, as "Labelling a media and a document" describes them. Each is optional and `--tag` can be repeated. A document must match everything that is given, the document included: the media matches the title of its media whatever the capitals, the author matches any one of its authors whatever the capitals, the category is `book`, `paper` or `other`, and each tag must be a tag of its media or one of its own tags. The filter holds for every item that is printed, the nearest items, the items that the graph adds and the items of step 6, and it works with `--kind` and with `--answer`. When no document matches, which is what a title or an id that no stored document has gives, the command prints `no items found` and the question is not embedded.

`--answer` asks the model of the Ask choice to write an answer from the items that were found. It runs through the `claude` command on your subscription, with no tools, and it is not started while `ANTHROPIC_API_KEY` is set. It makes one call. Before the search, the command checks that the key is not set and runs `claude auth status` once, which is free and asks no question, so nothing is embedded when `claude` cannot answer: it prints `the agent of the Ask choice cannot write an answer, so nothing was searched for` and the reason. The model is given the question and the items, numbered: the document, the page (for a code listing that spans pages, its first and last page, such as `155 to 159`), the kind, the label, the language of a code listing, and the text, the raw LaTeX of a formula, the code as printed, or the explanation of a figure with the path of its picture. It replies with claims, and each claim names the numbers of the items that support it. A claim writes a symbol as LaTeX between `\(` and `\)`, and a name from code, such as `push_market_tick`, between backticks. When the model writes such a name as LaTeX all the same, the program turns it into code: LaTeX sets only one character after `_` below the line, so a math span of only letters, digits and underscores with two or more characters after an underscore can only be a name from code. The command prints the answer, not the results, then a blank line, one line for the tokens of each model that the question used (the embedding and the model of the Ask choice) and `cost: ≈ $<cost>`. The tokens and the seconds of the `claude` call also go to standard error. After each claim it prints one line for each source: the title of the document (which names the book and the chapter, for a chapter), the printed page (for a code listing that spans pages, `page 155 to 159`), and the kind and label of the item. Under the line of a formula, or of a code listing, it prints the LaTeX, or the code, exactly as the document has it, and under the line of a figure it prints the path of the picture. The program writes the citations, the LaTeX and the code, not the model, so a title, a page, a formula or a listing is never retyped. A reply with a claim that names no source, or an item that was not given, is an error, and the question is not asked again. When nothing is found, the command prints `no items found` and does not ask the model. When the model replies with no claim, the command prints `the stored items do not answer the question`. The prompt and the JSON Schema of the reply are in `crates/rag-retrieval/src/answer/prompts/`.

## Building a training dataset

```bash
cargo run --release -p rag-dataset -- build --name smoke --dry-run
cargo run --release -p rag-dataset -- build --name small --document "Option Volatility and Pricing, chapter 1: Sample Pages" --rewrites 1 --questions 1 --max-sft 5
```

`rag-dataset build` makes a training dataset for Gemma 4 from the documents that are converted under `CONTENT_DIR` (`content` unless set): whole sections of the books for continued pre-training, texts that a model rewrote from them, questions with answers that were checked, and a manifest that says what the dataset holds. It converts nothing again and writes to neither store. It reads the graph, to see how rarely the library mentions the concepts of each section, unless `--no-graph` is given. The model is the Datasets choice of model and thinking. How to train on the dataset is in [training.md](training.md). The same build is the **Datasets** tab of the desktop app (see "The desktop app") and the `build_dataset` tool of the MCP server, which [mcp.md](mcp.md#5-building-a-training-dataset) describes.

The first command is a dry run: it exports, checks and plans, asks no model, and costs nothing. Its summary says how many texts, questions and model calls a full build would make, so run it first. The second is a small paid build of one chapter: it uses the agent of the Datasets choice, in time and in subscription usage.

### The flags

| Flag | What it gives |
| --- | --- |
| `--name <name>` | Required. The name of the dataset: letters, digits, `-`, `_` and `.`, and not `.` or `..`. It names the dataset's folder in `DATASET_DIR` |
| `--media <title>` | Build from this media only, whatever the capitals. Repeat it for more. Without it every media is built |
| `--document <id or title>` | Build from this document only: the document id that an ingest prints, or its whole stored title. Repeat it for more. Without it every document of the chosen media is built |
| `--holdout <id or title>` | Keep this document out of training for evaluation. Named like `--document`. Repeat it for more |
| `--include-book-parts` | Also build from the parts of a book that are not chapters, such as its index. A part with "appendix" in its title is always built |
| `--rewrites <n>` | How many rewritten texts each training section gets from each passage of it, before the rarity of its concepts scales the number. 3 when left out |
| `--questions <n>` | How many questions each section gets, before the rarity scales the number. 2 when left out |
| `--max-sft <n>` | The most fine-tuning questions to ask of the training sections, so the most examples to write. 300 when left out |
| `--max-synthetic <n>` | The most synthetic texts to write. Without it the plan decides |
| `--samples <n>` | How many times the solver answers each question to think about. 3 when left out |
| `--no-run-code` | Do not run the Python that a model wrote to check an answer |
| `--dry-run` | Export, check and plan only: ask no model, and print what a full build would ask |
| `--min-page-score <share>` | Reject a section whose worst page matches less of its text layer than this share, from 0 to 1. 0.90 when left out |
| `--keep-unrendered` | Keep a section whose formulas do not render, instead of rejecting it |
| `--no-graph` | Do not connect to the graph. The plan then ignores how rare a concept is |
| `--out <folder>` | The dataset's own folder, where its files go. Without it, `DATASET_DIR/<name>` |

### The folder and the files

The files of a dataset are in `DATASET_DIR/<name>` (`datasets/<name>` unless `DATASET_DIR` is set), or in the folder that `--out` names:

| File | What it holds | Written by | In a dry run |
| --- | --- | --- | --- |
| `cpt/raw.jsonl` | The kept sections of the training documents, as the book prints them | The checks, after the plan | Yes |
| `cpt/heldout.jsonl` | The kept sections of the documents that are held out | The checks, after the plan | Yes |
| `cpt/rejected.jsonl` | The sections that the checks kept out, each with its reason | The checks, after the plan | Yes |
| `cpt/synthetic.jsonl` | The texts that a model rewrote and a judge kept | The synthetic texts | No |
| `sft/train.jsonl` | The checked questions and answers of the training documents | The fine-tuning examples | No |
| `sft/heldout.jsonl` | The checked questions and answers of the documents that are held out | The fine-tuning examples | No |
| `sft/rejected.jsonl` | The questions that failed a check, each with its reason | The fine-tuning examples | No |
| `plan.json` | What the plan asks the models to write for each section | The plan | Yes |
| `manifest.json` | What the dataset holds, with the hashes of its files | The manifest | Yes |

The three files of the sections hold what the checks decided, and they are written once the plan is made, so that each record carries the concepts that the plan found (`source.concept_ids`). A file is written whole or not at all: it is written beside its name and then renamed, so a run that is killed leaves no half-written file. `cpt/raw.jsonl`, `cpt/heldout.jsonl` and `cpt/rejected.jsonl` are written even when they are empty.

A build first checks the name and the selection and, unless it is a dry run, that the model is ready. Only then does it remove the nine files of an earlier build in the same folder, and nothing else in it, and write its own. A name that cannot name a folder, a media or a document that no converted document matches, and a model that is not signed in stop the build before anything is removed.

While it runs, the command prints one line on standard error for each stage, so that standard output holds only the summary at the end. The log lines of `rag_core` and `rag_dataset`, such as the tokens of each model question, go to standard error too.

| Line | When |
| --- | --- |
| `exporting <n> documents` | The sections of the chosen documents are cut |
| `checking the sections` | The checks run |
| `planning the generation` | The plan is made |
| `writing <n> synthetic texts` | The synthetic texts start. Not in a dry run |
| `writing <n> SFT examples` | The fine-tuning examples start. Not in a dry run |
| `writing the manifest` | The manifest is written |

### Which documents

A document is exported when it is finished, it is chosen by `--media` and `--document` (all are when neither is given), and it is not a part of a book that is left out. What a document is:

| Role (`doc_role`) | What it is |
| --- | --- |
| `chapter` | A chapter of a book |
| `part` | A part of a book that is not a chapter, such as its index. A titled document in a media that has chapters |
| `document` | A whole work with no chapters, such as a paper |

A `part` is exported only with `--include-book-parts`, or when its title has "appendix" in it, whatever the capitals. A document that is not finished is left out. A document whose `chapter.json` cannot be read is left out too, and named in a warning. A finished document whose pages cannot be read stops the build with `could not read the chapter in <folder>; convert it again`, once the files of an earlier build are removed.

`--media` takes the title of a media, `--document` and `--holdout` take a document id or a whole stored title, such as "Option Volatility and Pricing, chapter 1: Sample Pages". The capitals do not matter, nor the space at the ends of a title. An entry that matches nothing in the content folder stops the build before anything is written: a misspelt hold-out would otherwise be trained on. The documents are taken in the order of the library. A document named by `--holdout` has the `split` `heldout`: its sections go to `cpt/heldout.jsonl`, it gets no synthetic text, and each of its sections gets at most two questions, which go to `sft/heldout.jsonl`. `--holdout` does not choose a document: with `--media` or `--document`, a held-out document must be chosen by them too, or it is not built.

### Sections

A section is the unit of the dataset. `section/` cuts the headings of a converted chapter into sections that are neither too short to teach anything nor too long to train on whole:

- A heading with everything under it is one section when it comes to at most 8,000 estimated tokens. When it is longer, the text directly under the heading is one section and each heading under it is cut the same way.
- A section of fewer than 500 tokens is joined to the next one when that sits under the same parent heading, and otherwise to the one before it. A short section with no neighbour under the same parent stays short.
- A section that is still over 8,000 tokens is cut into parts at the edges of blocks, never between a text and the formula it leads into. A single block over the limit is a part by itself, and a footnote that no text carries goes last in its part.
- The id of a section is `<media folder>:<document folder>:s001`, counted from 1 in reading order, with `:p1` after it for the first part of a section that was cut in parts.
- The text opens with the heading line, `# <media> › <chapter name or document title> › <heading> › …`, where a paper that is named after its media says the name once. Then come the blocks of the section in reading order, each heading at its own rank.
- Inline maths is `$…$`. The converter marks maths with `\(` and `\)`, so a `$` in the text of a page is a dollar sign, and it is written `\$`. A formula is a `$$` block, with its printed label on the next line in parentheses.
- Code is fenced with its language, in a fence longer than any run of backticks inside it. A table is its saved Markdown text, under a line with its label and caption. A footnote is `[^marker]: text`, or `Footnote: text` without a marker, and it follows the text that shows its marker.
- A figure is `[Figure <label>: <caption> — description (model-written): <explanation>]`, and the picture, when the figure was cut out of the page, is listed in `images` with the offset in characters of its `[`. What the model wrote is counted apart from what the book printed in `provenance`.
- The running header and the printed page number are not in the text. A paragraph that a page break cut is joined again, and a word cut at a hyphen is joined without it. The text is in Unicode NFC.
- A size is estimated at four characters to a token, rounded up, and every record says so with `tokens_estimated`.

A section also has notes, which only the models that write from it read: what each formula is called, what it states and what its symbols mean, and the words that each figure prints. They are not in `text`.

### The checks

`checks/` runs every check on every section, whatever the first one finds, so that a rejected section still shows what each check made of it. The section is rejected for the first of these that applies:

1. **Page gate** (`page_score`). The score of a section is the lowest `piece-words-in-text-layer` of its pages, which the conversion saved for each page: the share of the words of the page that its text layer also holds. A section takes the score of its worst page. The score is null for the whole section as soon as one of its pages was written by hand and has no conversion record, has the route reason `text-layer-almost-empty`, or cannot be read, and a section with a null score is kept. A section is rejected when its score is under `--min-page-score`.
2. **Formulas** (`latex`). Every `$…$` and `$$…$$` outside the code listings is read with `ratex_parser`, the LaTeX parser of the typesetter that the desktop app draws formulas with. `latex_renders` is null when the section has no maths, and false when a formula does not parse or is opened and not closed. A section with `latex_renders` false is rejected, unless `--keep-unrendered` is given.
3. **Copies** (`duplicate_of <id>`). `dedup.sha256` is the SHA-256 of the text of the section after its heading line, so the same text under another title is found. A section with the hash of a section that was kept is rejected as `duplicate_of <the id of the first>`. Only a kept section is remembered, so a copy of a rejected section is judged on its own pages. `dedup.minhash_cluster` is always null: near-duplicates are not looked for.

`code_parses` is the result of compiling every `python` listing with `python3 -I -m py_compile`. It is null when the section has no Python listing or `python3` cannot be run, and false when a listing does not compile. It is recorded and never rejects a section.

A rejected section is written to `cpt/rejected.jsonl` as the record it would have been, with `rejected` added: `page_score`, `latex` or `duplicate_of <id>`.

The default of `--min-page-score` is 0.90. The page gate compares the copy with the text layer of the PDF, and inline maths counts as words that the text layer prints as symbols, so a good page of maths matches less than a page of prose. The seven pages of the sample chapter score 0.995, 0.974, 0.970, 0.943, 0.968, 0.997 and 0.913, and its one section scores 0.913, the lowest. A section inherits its worst page, so one weak page keeps a whole section out: change the default with a dry run in hand: its summary counts the sections that each reason rejects, and `cpt/rejected.jsonl` holds them.

### The plan

The plan decides, for each kept section, what the models are asked to write. It is written to `plan.json`:

| Field | What it holds |
| --- | --- |
| `graph_read` | False when the build did not read the graph. Then no section has concepts or relations |
| `sections[].section_id` | The section |
| `sections[].concept_ids` | The concepts that the stored items of the section mention |
| `sections[].passages` | How many passages the section is cut into for the writers |
| `sections[].rewrites` | How many texts of each kind to write from each passage. The explanations of relations are counted once for each relation |
| `sections[].relations` | The relations to explain, the rarest first: `kind`, the two concepts `from` and `to`, each with its `id`, `name` and `definition`, and the `item` that stated the relation |
| `sections[].questions` | How many questions to ask |
| `totals` | `texts` by kind, `questions`, and `model_calls` |

The rules, for a training section:

- The rarity factor says how rarely the library mentions the concepts of the section. It is the median, over the concepts of the section, of the typical mention count divided by the mention count of the concept. The typical count is the median, over the concepts that the training sections name, of the number of items that mention a concept, and a concept that two sections name counts twice. The factor is clipped to 0.5 to 3.0. It is 1.0 for a section with no concept, for a held-out section, and when no graph is read.
- Rephrased texts: `round(rewrites × factor)` for each passage, taking turns at `rephrase_wiki`, `rephrase_lecture` and `rephrase_qa`.
- Relations: up to `round(rewrites × factor)`, the rarest first, whatever the passages. Only a relation that an item of a training section stated is used.
- One exercise from each passage when the factor is at least 1.
- One notation variant from each passage, for about one section in twenty (picked by a hash of its id) that has a display formula.
- Questions: `round(questions × factor)`.
- A held-out section gets no texts and `min(questions, 2)` questions.
- Each kind but the relations is written once from each passage of the section. A passage is the unit the writers read (see "The writers"), so a section of several passages gets that many times the texts.

`--max-sft` and `--max-synthetic` cut the totals. The training sections are filled in the order of rarity, the rarest first, and a tie is settled by a hash of the section id, so that a cut is spread over the library and is the same on every run. A section keeps whole passages: a kind keeps a count for each passage or none, and what a section cannot use passes to the next. The questions of held-out sections are outside the cap.

The totals count the texts of each kind over every passage and the questions. `model_calls` is an estimate of what the build asks: two for each text (the writer and the judge) and `--samples` plus 2 for each question. A question that has a straight answer needs one solver call, and a failed question is asked once more, so the real number differs.

### The writers

Eleven roles write and judge. Each has an id and a version, which `prompt_versions` lists as `id@version`. The prompts are in `crates/rag-dataset/src/generate/prompts/` and `crates/rag-dataset/src/sft/prompts/`.

| Role | Is given | Writes |
| --- | --- | --- |
| `rephrase_wiki@1` | A passage | The section as an encyclopedia article |
| `rephrase_lecture@1` | A passage | The section as a lecturer's notes |
| `rephrase_qa@1` | A passage | The section as questions and answers |
| `exercise@1` | A passage | Two to four exercises with worked solutions |
| `notation@1` | A passage | The section with its quantities named by other symbols |
| `entigraph@1` | One relation and the passage that names its concepts most | An explanation of how the two concepts are related |
| `judge@1` | A passage, the kind of text and the text | The unsupported claims, whether the text is faithful, and a score from 1 to 5 |
| `questions@1` | A section and how many questions to write at most | Questions, each with a task, a difficulty and a seed |
| `solve_think@1` | The task, a question and its section | The working, the answer and the final value |
| `solve_direct@1` | The task, a question and its section | The answer, straight out |
| `judge_answer@1` | A section, a question, the working and an answer | Whether the answer is faithful, its unsupported claims and a score from 1 to 5 |

A passage is at most 2,500 estimated tokens of the section's own text, its heading line included, so that no model is asked to rewrite more than it writes well. It is cut at the edges of paragraphs and never inside a code listing, a display formula or the note of a figure. A single paragraph or block that is longer is a passage by itself. The notes of the section are put under the text of every passage. When one kind is asked for more than once from a passage, the later asks end with the line `Version 2`, `Version 3` and so on, so that the writer writes another text.

Each text is a draft that is written and judged on its own. The judge reads the draft against the passage it was written from, and the draft is kept only when the judge finds it faithful, finds no unsupported claim in it, and scores it 3 or more. A draft that is turned down, or whose writer or judge gave no answer, is left out and counted by the id of the role that wrote it, such as `notation`. The writers and the judges are asked four questions at a time. The texts are written to `cpt/synthetic.jsonl` in the order of the plan.

### The fine-tuning examples

The question writer is asked once for each section that has questions, for up to as many as the plan says, and may write fewer. Each question has a task, a difficulty from 0 to 1, and a seed: `book_worked_example` or `book_exercise` when the question turns an example or an exercise of the section into a question, otherwise `generated`, with the quote of the section that the question rests on.

| Task | Reasoning | How the answer is checked |
| --- | --- | --- |
| `derivation` | `think` | `agreement+judge` |
| `calculation` | `think` | `agreement+judge` |
| `code_pot` | `think` | `python_exec`, or `judge` with `--no-run-code` |
| `explain` | `direct` | `judge` |
| `compare` | `direct` | `judge` |

A `think` question gets `--samples` tries. Each try has its working, an answer and a final value. A `direct` question gets one straight answer. The three methods:

- `agreement+judge`: more than half of the tries asked agree on the final value. Two numbers agree when they differ by at most a relative 1e-3, and anything else agrees when it is the same text once the spaces and the dollar signs are gone and the capitals are lower case. The example is made from the shortest try of the biggest group that agrees. Then the judge must find the answer faithful, with no unsupported claim, and score it 4 or more.
- `python_exec`: the first fenced `python` block of each try is run as `python3 -I program.py`, in a new temporary folder that is removed afterwards, with only `PATH` in its environment, no input, and a limit of 10 seconds. A try passes when the program ends well and its output holds the final value of the try, and a try with no final value passes when the program ends well. No judge is asked. The example is made from the shortest try that passed.
- `judge`: the judge alone, which must find the answer faithful, with no unsupported claim, and score it 4 or more.

The generated Python runs with the rights of the person who builds: it is not sandboxed, so it can read their files and use the network. Only its environment, its folder and its time are limited. Build with `--no-run-code`, or `run_code: false` over MCP, to keep the build from running any. Then a `code_pot` question is checked by the judge alone. So is one when `python3` cannot be run, and the build says so in a warning.

A question that passed goes to `sft/train.jsonl`, or to `sft/heldout.jsonl` when its document is held out. One that failed goes to `sft/rejected.jsonl` with `rejected`, which is one of `no_answer` (no solver gave an answer), `disagreement` (too few tries agree), `judge` (the judge found the answer wrong, or gave no verdict) or `python_exec` (no program printed the value). A rejected record whose question got no answer has the user message alone. When the question writer gives no answer for a section, its planned questions are counted as `no_answer` and have no record.

### Kept answers

Every good answer of a model is kept as a file in `DATASET_CACHE_DIR` (`data/dataset-cache` unless set). The name of the file is made from the role and its version, the model with its provider and thinking level, the prompt, the schema and the input, so a second build of the same sections asks nothing, and a change of prompt version, model or thinking level asks again. A question that fails is asked once more. A reply that does not fit the schema counts as a failed try, and a failed question is not kept, so the next build asks it again.

Some failures stop the build at once, and are not tried again: a usage limit, a sign-in that is lost, `ANTHROPIC_API_KEY` set, a `claude` that cannot start, model settings that cannot be used, and any failure of a Codex model. The files of the stage that stopped and of the stages after it, and `manifest.json`, are not written; the files of the sections and `plan.json` are. The answers given before the stop are kept, so the same command goes on without paying twice. When the folder of kept answers cannot be made or used, the error names `DATASET_CACHE_DIR`.

### The records

Every file of `cpt/` holds one record on each line, with the fields below. The names are the names in the files.

| Field | What it holds |
| --- | --- |
| `id` | `<section id>:raw` for a section. For a synthetic text, `<section id>:<kind>:<n>`, where `n` counts the drafts of that kind in the section from 1, so a draft that was turned down leaves a gap |
| `kind` | `raw`, `rephrase_wiki`, `rephrase_lecture`, `rephrase_qa`, `entigraph`, `notation_variant` or `exercise` |
| `source` | Where it came from: `media`, `document_id`, `document` (the stored title), `doc_role`, `section_path` (the headings as saved), `pages` (positions in the PDF, from 1), `item_ids` (the stored items of the section) and `concept_ids` |
| `text` | Markdown with the maths between dollars |
| `images` | The pictures of the section: `ref` (the file) and `pos` (the offset in characters of the figure's note). Empty for a synthetic text |
| `n_tokens` | The estimated tokens of `text` |
| `tokens_estimated` | Always true |
| `provenance` | `transcribed_tokens` (what the book printed) and `model_written_tokens` (the explanations of figures, which a model wrote from the picture). For a synthetic text all the tokens are model-written |
| `quality` | `page_score`, `latex_renders`, `code_parses` and `judge_score`, each null when its check was not run or does not apply. A synthetic text has `judge_score` only |
| `dedup` | `sha256` and `minhash_cluster` (always null). For a synthetic text the hash is the hash of its text |
| `gen` | How a model-written text was made: `model`, `prompt_id` (such as `rephrase_wiki@1`) and `faithfulness_checked`. The three fields are null for a raw section |
| `split` | `train` or `heldout`. A synthetic text is always `train` |
| `pipeline_version` | The version of `rag-dataset` that wrote the record |
| `rejected` | In `cpt/rejected.jsonl` only: the reason |

A line of `sft/` has these fields:

| Field | What it holds |
| --- | --- |
| `id` | `sft:<section id>:0001`, the number counted from 1 in the order the question writer wrote the questions of the section |
| `task` | `derivation`, `calculation`, `explain`, `compare` or `code_pot` |
| `reasoning` | `think` or `direct` |
| `messages` | The turns, each with `role` (`user` or `assistant`), `content` and, on the answer of a `think` example, `thinking`. There is no system turn and no thinking marker: [training.md](training.md) says how to render them |
| `seed` | What the question rests on: `kind`, `item_ids` (the stored items of the section), `section_path` and `quote` |
| `verification` | `method` (`agreement+judge`, `python_exec` or `judge`), `passed`, `n_teacher_samples` (how many tries were asked), `agreement` (the tries that agree, the programs that printed the value, or 1 for a judge that passed) and `judge_score` (null when no judge gave a verdict: always for `python_exec`, `no_answer` and `disagreement`) |
| `known_by_cpt_model` | Always null |
| `difficulty` | From 0 to 1 |
| `ifd` | Always null |
| `split` | `train` or `heldout` |
| `pipeline_version` | The version of `rag-dataset` that wrote the record |
| `rejected` | In `sft/rejected.jsonl` only: `no_answer`, `disagreement`, `judge` or `python_exec` |

### The manifest

`manifest.json` says what a build made. It is pretty JSON, with these fields:

| Field | What it holds |
| --- | --- |
| `name` | The name of the dataset |
| `created_at` | Seconds since 1970-01-01 UTC. The workspace has no date crate, so the time is a plain count. It is not part of `sha256` |
| `pipeline_version` | The version of `rag-dataset` |
| `prompt_versions` | The prompts a full build asks, as `id@version`: the seven roles of the synthetic texts, then the four of the fine-tuning examples. A dry run lists them too |
| `options` | The options of the build, as `BuildOptions` writes them: `name`, `out`, `content_folder`, `cache_folder`, `selection` (`media`, `documents`, `holdout`), `include_book_parts`, `rewrites`, `questions`, `max_sft`, `max_synthetic`, `samples`, `run_code`, `generation` (false for a dry run), `min_page_score` and `keep_unrendered` |
| `documents[]` | Each exported document: `document_id`, `title`, `doc_role`, `split`, `sections` (the sections that were kept) and `n_tokens` (of those sections) |
| `files[]` | Each file of records that exists: `path`, `sha256`, `samples` (its lines) and `n_tokens` (the sum of the `n_tokens` of its lines, 0 for a file whose lines have none) |
| `sha256` | The version of the dataset: the SHA-256 of one line `<file sha256>  <path>` for each entry of `files`, in order, written as `sha256sum` prints them. Two builds that wrote the same files have the same version, whenever they ran |
| `mix` | `raw` and `heldout` (the kept sections by split), `synthetic_by_kind` (the synthetic texts written, by kind, such as `notation_variant`), `sft_by_task` (the kept examples by task, training and held out together), `think` and `direct` |
| `rejected` | `sections` (`page_score`, `latex`, `duplicate_of`), `synthetic` (the drafts turned down or not answered, by the id of the role that wrote them, such as `notation`) and `sft` (`no_answer`, `disagreement`, `judge`, `python_exec`), each a count for each reason |
| `judge_scores` | `synthetic` and `sft`, each with the keys `"1"` to `"5"` always present. They are read back from the files: `quality.judge_score` of `cpt/synthetic.jsonl`, and `verification.judge_score` of the three files of `sft/`. A record with no judge score is not counted. Only kept synthetic texts have a record, so `"1"` and `"2"` are always 0 for `synthetic`; a draft that was turned down shows in `rejected.synthetic` |
| `usage[]` | One entry for each model that was asked: `model`, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens` and `estimated` |
| `cost_usd` | About what the build cost in US dollars at the prices of "Costs", or `null` when a model has no price. 0 for a dry run |
| `notes` | What the dataset does not do, in plain sentences: no near-duplicates removed, estimated token counts, no packing, no sympy checks, `ifd` and `known_by_cpt_model` null, no decontamination, no replay data, and where the recipe is |

The counts of `mix` and `rejected.sections` are the counts of the summary below. The summary does not print `rejected.synthetic` or `rejected.sft`.

### The summary

When the build ends, it prints a summary on standard output, one line for each fact:

| Line | What it says |
| --- | --- |
| `dataset: <name>` and `folder: <folder>` | The dataset and where it is |
| `documents: <n> for training, <n> held out` | The exported documents by split |
| `sections: <n> for training, <n> held out, <n> rejected` | The kept sections by split, and the sections the checks kept out |
| `rejected: <reason> <n>, …` | Only when a section was kept out: the count for each reason |
| `planned texts: <kind> <n>, …` | The texts the plan asks for, by kind |
| `planned questions: <n>` | The questions the plan asks for |
| `planned model calls: <n>` and `dry run: no model was asked` | A dry run only |
| `synthetic texts: <kind> <n>, …` and `SFT examples: <task> <n>, … (think <n>, direct <n>)` | A build only: what was written. `SFT examples: none` when no example passed |
| `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write` | One line for each model that was asked, ending ` (estimated)` when its tokens are estimated |
| `cost of this run: ≈ $<cost>` | About what the models cost: `under $0.01` for a cost that would round to $0.00, and `unknown` when a model has no price, as a Codex model has none. A dry run prints `≈ $0.00` |

## The desktop app

```bash
cargo run --release -p gui
```

The program is `quanty`, built as `target/release/quanty`. It opens one window with four tabs, **Ask**, **Library**, **Ingest** and **Datasets**. Ask is one screen of six panels:

| Panel | What it shows |
| --- | --- |
| Ask bar | The question, the mode, and the filters Media, Document, Authors, Tags and Category |
| Answer | The written answer with its citations, and the results by kind: all, formulas, figures, tables, code |
| Source in Context | The page a citation stands on, with its figures, formulas, tables, code and concepts |
| Concept Graph | The concepts of the answer and how they link |
| Retrieval Path | The path of the search in five rows, with what each step produced |
| Follow up | Questions to ask next and a box for your own: each is a new search with the same mode and filters |

The Source panel has the tabs Page, Figures, Formulas, Tables, Code and Concepts, and the Answer panel has Answer, Results, Formulas, Figures, Tables and Code. A tab of a kind is there even when it holds nothing. A code listing is drawn as it was printed, in fixed-width type on a shaded bed, with its language at the right above it; a line that is wider than the room scrolls sideways. A secondary click on it offers **Copy code**. **Share** writes it as a fenced block with its language. A listing that page breaks cut is one result, and its row and its line in **Share** name its first and last page, such as `pp. 155–159`.

| Key | What it does |
| --- | --- |
| `⌘1`, `⌘2`, `⌘3`, `⌘4` | Go to Ask, Library, Ingest, Datasets |
| `/` or `⌘K` | Write a question (`Enter` asks it) |
| `J`, `K` | Next and previous result |
| `⌘.` or `Esc` | Stop the search or the answer |
| `Esc` | Put a maximised panel back, when no search or answer is running |
| `⇧⌘S` | Copy the answer with its citations |
| `⌘]`, `⌘[` | Next and previous page of the source |
| `⌘+`, `⌘−`, `⌘0` | Zoom the page while the pointer is over it |

`/`, `J`, `K` and `Esc` do this only while no text box has the keyboard.

Source in Context and Concept Graph each have **Maximise**, the corner brackets at the top right of the panel. It gives the panel the whole tab under the top bar: the Ask bar and the other four panels are not drawn, the page is fitted to the wider room, and the graph is drawn in the middle of the larger one. **Restore**, in the same place, puts every panel back where it was. So does `Esc` when no search or answer is running, a click on a tab, and `/` or `⌘K`, which also puts the keyboard in the question box. One panel is maximised at a time.

Two other buttons put a view back and leave the panels where they are. **Reset view**, the round arrow beside Maximise in Concept Graph, undoes a drag or a zoom of the picture. **Fit to width**, the two-way arrow at the right end of the zoom bar under the page, puts the zoom back to 100%.

It needs what `rag-query --answer` needs: Qdrant and FalkorDB, `EMBEDDING_GEMINI_API_KEY`, and `claude` signed in with `ANTHROPIC_API_KEY` not set. An ask makes one embedding call and one Sonnet call. In the mode **Results only** it makes the embedding call alone. When a service is not ready, the part of the window that needed it says which one and what to do.

Once the answer has landed, the row of tabs of the Answer panel says what the ask used, left of **Share**, such as `12k tokens, about $0.03`. In the mode **Results only** it says what the search used, such as `12 tokens, under $0.01`. In a window too narrow for it beside the tabs, it is left out.

The program reads `.env`, `content/` and `data/` from its home folder. It finds that folder in this order: the folder given with `--home <folder>`; the folder that `QUANTY_HOME` names; the nearest folder at or above the current one that holds a `.env`; the nearest folder at or above the program's own folder that holds one, which is what a start from Finder uses; and last the current folder, with no `.env`.

A citation opens its page when the document's folder is found. The document is looked for under the content folder, by its source file; a folder that is stored with the document is used first, when there is one. A document that is found in neither way shows "The page was not found" with what to do: this is what a document that was ingested from a folder outside the content folder shows.

The filters of the Ask bar are lists. **Media** offers "All media" and each media that has a document. **Document** offers "All documents" and the documents of the chosen media, in reading order: a chapter shows as "Chapter <number> · <name>", and any other document, such as a part of a book or a paper, by its own title. It is off until a media is chosen, and choosing another media clears it. With a document chosen, an ask looks at that document alone. **Authors** offers "All authors" and each author of a document, once. **Tags** lets you tick any number of tags, from the tags of the media and the own tags of the documents. **Category** offers "Any category", Book, Paper and Other. An ask looks only at the documents that match every filter, as the flags of `rag-query` do. The Source panel picks its page by **Media** and **Document**: a chapter shows as "Chapter <number> · <name>", and any other document by its title.

`--fixture <scene>` runs the whole window on built-in data from `samples/content`, with no store, no model and no cost. `--fixture list` prints the scenes: each is one state of the screen, such as `black-scholes` (a full answer), `stores-down` or `first-run`. The Datasets tab has three: `datasets-ready`, `datasets-running` and `datasets-failed`.

The **Library** tab shows each media as a card: its title, a badge for its category (Book, Paper or Other), its authors and its tags. A media that was saved and has no document yet says so. The documents that belong to no media, such as a picture that stands alone, are under "No media". The pencil **✎ Edit** at the right end of a card opens the form of the media in the card: three chips for the category, then **Authors** and **Tags**, each with commas between them. It has no title box, because the title names the media's folder and cannot change. **Save media** is on once something has changed. It writes the labels to the `Media` node, to every document of the media and to every point, the way `rag-ingest media` does, but it replaces every label, so the authors and the tags can also be emptied here. **Cancel** closes the form. A save that is refused keeps the form open and says why. **Delete**, beside **✎ Edit**, deletes the media with every document of it. The group "No media" has no **Delete**: its documents are deleted one by one.

In the card, each document shows its chapter, as "Chapter <number> · <name>", or its title; its pages and its items; and its own tags. The chapters of a book come first, by number, and then its other parts, by title. A part of a book shows its own title, such as "Index", without the book's title in front. A document whose ingest did not finish says that a search may miss parts of it. **Copy id** copies the document id. **Read** opens the document in the Source panel of Ask. **Edit tags** opens a box for its own tags, with commas between them, and **Save** writes them to both stores the way `rag-ingest tag` does. **Delete** deletes the document. Every edit is free: no embedding and no model call. No edit and no delete can be made while an ingest runs, or while another change of the library is on its way, and a **Delete** that is off says why when the pointer rests on it. A **Delete** is also off while a dataset is being built, because the build reads the documents that it would remove: it says "Nothing can be deleted while a dataset is being built." The pencil and **Edit tags** stay on during a build.

A sheet asks before any delete. It is titled "Delete the document <name>?" or "Delete the media <title>?", says what goes and that it cannot be undone, and offers **Delete document** or **Delete media**, and **Cancel**. `Esc` and a click beside the sheet are **Cancel**, and nothing is deleted. The delete does what `rag-ingest delete-document` and `rag-ingest delete-media` do. What goes: the passages, formulas, figures, tables and code listings of the document from the library, its converted pages from the disk, and the copy of its PDF that the MCP server saved under `content/_uploads`, when there is one. For a media, that goes for every document of it, and the media goes too, and the sheet says how many documents go with it. What stays: the PDF that you picked from your own disk, the concepts, and every other media and document. A media whose last document is deleted stays in the list, with no document. A document that is being converted at this moment is refused, and nothing of it is removed: wait for that run to end, and delete again. A delete that fails says why on its card, and the library is read again, because the documents of a media before the one that failed are already gone. **Delete** again finishes it. A check of a PDF on the Ingest tab is made again after a delete, because a delete changes what is converted and what is ingested. A delete costs nothing, but the same PDF ingested again is converted again, which is paid work.

The **Ingest** tab is the **Add media** journey, in three steps:

1. The media. The **Media** list holds every stored media, as "<title> (<category>)", and last **Add new media…**. A chosen media shows as a card with its title, its category badge, its authors and its tags. Its labels are fixed here, and its pencil **✎ Edit** opens the same form as in the Library. **Add new media…** opens the form of a new media: the category chips, **Media title**, **Authors** and **Tags**, then **Save media** and **Cancel**. A saved media is chosen at once, and it stays in the list each time the app starts, also before its first PDF. A title that the library already has, whatever its capitals, is refused: choose that media from the list.
2. The PDF. **Choose a PDF** opens a file dialog. For a book, **Part of the book** offers **Chapter** and **Other part**. A chapter takes **Chapter number** and **Chapter name**, which are filled in from a file named `chapter-<number>-<name>.pdf` and else typed. **Other part** is for a part that is not a chapter, such as the index or the glossary: it takes a **Title**, filled in from the words of the file name when the file is not named as a chapter (`maths_appendix.pdf` gives "Maths Appendix"), and the PDF can have any name. The document is stored as "<book>, <title>". Choosing a file never switches the choice by itself, except that a file named as a chapter goes back to **Chapter**. The PDF of a paper or another media takes a **Title**, filled in with the title of the media. **Tags for this PDF** are its own tags; the tags of the media apply as well.
3. The check and the ingest. The check runs by itself once a media and a PDF are chosen and the fields are filled, and again when one of them changes. It does not run on each key: a box is read once it no longer has the keyboard. While it runs, a bar moves. It is free: it counts the pages of the PDF, says how many are converted already, and shows a notice for what would stop a start, such as `claude` not signed in. For a PDF that is ingested already it says "Already ingested with N items: a start converts nothing and pays for nothing." when the chapter's `insights.json` is saved, and also for a part of a book that is not a chapter, which gets no insights, and "Already ingested with N items, but its insights are not saved yet: a start pays only for the insights, takeaways and cheat sheet." when it is not, and then it also checks that the agent is signed in. **Start ingest** is paid work, the same as `rag-ingest pdf`. While the ingest runs, a bar shows the stage: "Connecting to the stores", "Checking what is already stored", "Opening the PDF", "Preparing the pages — 7 of 12 ready" while each page is cut out, and "Converting the pages — 3 of 12 done" once the models work; for the pages the words end with ", about N%", an estimate that runs from the first page cut out to the last page saved, with pages that an earlier run saved counted as done. Once pages are converted, the tokens and the cost so far are beside the words, such as `48k tokens, about $0.42 so far`. The later stages are "Writing the graph", "Embedding N items", "Storing the items", "Writing the insights, takeaways and cheat sheet", "Reading the concepts — N of M" and "Linking the concepts — N of M" (the last two also end with ", about N%"). Keep the app open while it runs. If it stops, start the same PDF again and it carries on. At the end, a notice says what was ingested, the tokens of each model and the cost of the run, and **Add another PDF** clears the PDF and keeps the media chosen.

The **Datasets** tab (`⌘4`) builds a training dataset, as "Building a training dataset" describes. Under the title "Build a training dataset" it has a form:

| Field | What it holds |
| --- | --- |
| **Dataset name** | Letters, digits, `-`, `_` and `.`. A blank name keeps **Start build** off, and the pointer on it reads "Type a name for the dataset first." |
| **Media** | "All media", or one media that has a document |
| **Hold out** | A box of ticks for the documents that have a folder, to keep out of training. "None held out" when none is ticked. Under "All media" each document is labelled "<media>, <chapter or title>" |
| **Rewrites per section** | A stepper from 0 to 10 in steps of 1. 3 at the start, as `--rewrites` |
| **Questions per section** | A stepper from 0 to 10 in steps of 1. 2 at the start, as `--questions` |
| **Most SFT examples** | A stepper from 0 to 2000 in steps of 50. 300 at the start, as `--max-sft` |
| **Dry run (export, check and plan only; asks no model)** | A box. Ticked, the build is free |

Every other option has the default of `rag-dataset build`. The build reads the graph also in a dry run, so a graph that is down fails it.

**Start build** is paid work, the same as `rag-dataset build`, unless the dry-run box is ticked: the line under it says "Paid work: the selected model writes the rewrites and the answers; a dry run is free." The model is the Datasets choice, which the top bar shows on this tab. There is no Stop button. **Start build** is off while a delete is on its way. An ingest and a build can run together.

While a build runs, the form is off and a bar shows the stage in words: "Starting the build", "Exporting the documents — 3 of 12", "Checking the sections", "Planning what to write", "Writing the synthetic texts — 6 of 12", "Writing the fine-tuning examples — 5 of 40" and "Writing the manifest". The stages that count end with ", about N%", such as "Writing the synthetic texts — 6 of 12, about 50%". Once the models have been asked, the tokens and the cost so far are beside the words, such as `31k tokens, about $0.12 so far`. Under the bar it says to keep the app open: "If it stops, start the same build again: answers already written are not paid for twice."

At the end a notice says "Built <name>" with the counts, such as "2 documents, 4 sections for training and 2 held out, 1 rejected. 12 synthetic texts and 8 fine-tuning examples (5 think, 3 direct)." For a dry run it ends "Dry run: 70 model calls planned, none made." Then come the tokens of each model, the cost, and "Saved in" the folder of the dataset, `DATASET_DIR/<name>` made absolute. A build that failed shows "The build failed" with what to do. **Try again**, after a failure, starts the build that the form holds now. **Build another** clears the notice and brings **Start build** back.

Not built yet: the notices tray, the help sheet and the health check.

## The MCP server

`quanty-mcp` lets an AI agent search the stored library, read the pages of its documents, get a cited answer, send a PDF to be ingested and build a training dataset, over the Model Context Protocol. `build_dataset` starts a build, and `dataset_status` gives its stage, its usage and its cost while it runs. How to build and start it, its tools and their costs are in [mcp.md](mcp.md).

## Costs

Every ingest and every question says what it used: the tokens of each model, and about what they cost.

| Where | What it shows |
| --- | --- |
| `rag-ingest <folder>`, `rag-ingest <picture>` and `rag-ingest pdf` | At the end of the summary, one line for each model, `tokens of <model>: <input> in, <output> out, <cache read> cache read, <cache write> cache write`, then `cost of this run: ≈ $<cost>` |
| `rag-ingest pdf` while it runs | `page <n> converted ($<cost>)` for each page |
| `rag-ingest insights` | `written: …`, then one line for each model and `cost of this run: ≈ $<cost>`. A dry run asks no model and prints no cost |
| `rag-dataset build` | At the end of the summary, one line for each model that was asked, then `cost of this run: ≈ $<cost>`. A dry run asks no model and prints `cost of this run: ≈ $0.00` |
| `rag-query` and `rag-query --answer` | The same lines after the results or the answer, then `cost: ≈ $<cost>` |
| The desktop app | `48k tokens, about $0.42 so far` beside the running ingest, and the same beside a running dataset build; the tokens of each model and the cost in the notice at the end of each; `12k tokens, about $0.03` in the row of tabs of the Answer panel |
| The MCP server | `usage` and `cost_usd` in `search`, `answer`, a `done` report of `ingest_status`, and every report of `dataset_status`, as [mcp.md](mcp.md) shows |

A cost is the tokens of each model times the price of that model in the table below, added up. A model that the table does not have is counted at what `claude` reported for it. When a model has neither, the cost is `unknown` (`null` in the MCP server). A Codex model has no price, so the summary of a dataset build on one ends with `cost of this run: unknown` after its tokens, while the running line of the desktop app shows its tokens alone, such as `31k tokens so far`. A cost that would round to nothing shows as `under $0.01`.

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

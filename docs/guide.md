# Using quanty

How to choose a model and a database, where a library lives, how to add a PDF, and how to use the desktop app, the command line and the costs. To install and start quanty, see the [README](../README.md).

- [Choose a model](#choose-a-model)
- [Choose a database](#choose-a-database)
- [Where a library lives](#where-a-library-lives)
- [Add your first PDF](#add-your-first-pdf)
- [The desktop app](#the-desktop-app)
- [Command line](#command-line)
- [Costs](#costs)
- [Start fresh](#start-fresh)
- [Artifact descriptions](#artifact-descriptions)

## Choose a model

The **Model** and **Thinking** dropdowns in the desktop top bar are for the tab on screen. Ask,
Ingest and Datasets each keep their own Model and Thinking choice, which the top bar shows on that
tab; Library has none. Ask is for answers, Ingest for page conversion and concept extraction, and
Datasets for dataset builds. The default is **Codex / GPT-6.1 Sol / Medium**. Claude models remain
in the same menu. The controls use the app's existing dropdown styling and save immediately;
finish any running operation before switching.

All language-model calls run headlessly through the installed CLI. Run `codex login` once
with your ChatGPT account, or sign in with `claude` when using Claude. No OpenAI API key is
needed. Codex CLI 0.162.1 is tested; it must support `--ignore-user-config` and `--ignore-rules`.

The saved choices are `.quanty-model.json` in the Quanty home folder. The file holds `ask`,
`ingest` and `datasets`, one choice for each kind of work, each with `provider`, `model` and
`thinking`. The desktop app, command line tools, OCR and MCP server read it before creating a model
client, so no restart is needed. `rag-query` and the MCP `search` and `answer` tools use the Ask
choice. `rag-ingest` (concepts, the insights of a chapter, `pdf`, `insights` and `health`), page
conversion (`ocr` and the app's ingest) and the MCP `ingest_pdf`, `write_insights` and `health`
tools use the Ingest choice. `rag-dataset` and the MCP `build_dataset` tool use the Datasets choice.
A file saved by an older version of quanty holds one choice and gives it to all three until a tab is
changed; the next save writes the three choices. An MCP server that is still running from an older
version must be restarted. Without that file, `LLM_PROVIDER`, `LLM_MODEL` and `LLM_THINKING` in the environment or
`.env` set the defaults, the same for all three. A saved UI choice takes precedence. Codex defaults
to `gpt-6.1-sol` with `medium` thinking. Supported levels are `low`, `medium`, `high`, `xhigh`,
`max` and, for models that support it, `ultra`. `CODEX_BIN` can name a custom Codex executable.

PDF pages are rendered locally and attached as images, with their text layer, to headless
Codex. No model shell tools are needed. Gemini embedding and Jev conversion settings keep
their existing roles. Codex usage records tokens; dollar cost is unknown when no price is
available. [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode)
describes the CLI protocol.

## Choose a database

A database is the place a library lives. It is three things: the Qdrant store, the FalkorDB store, and the content folder that holds the converted pages. The default is **Local**: the two Docker stores on this machine and the `content/` folder of the quanty folder. You can choose a pair of stores on a server instead, and then two machines that choose the same database see the same library. [server.md](server.md) sets a server up, step by step.

At the right end of the footer, on every tab, an entry says which database is in use. It reads **Database: not chosen** when none is, and **Database: Office · connected** when one is, such as one called Office. After the name it can also say `not checked`, `checking…` or `down`. A click on it opens the sheet.

- **The first start.** When no database has been chosen yet, the sheet is the first thing the window shows, and nothing loads until you press **Use**. **Local** is the first choice in the list. It cannot be skipped: **Esc** and a click beside the sheet do nothing, and **Cancel** is off.
- **Every later start.** The app starts on the database you chose, checks it, and the footer entry says how the check ended. No sheet opens.
- **The sheet.** The **Database** list holds Local, each database you saved, and **New…**. A saved database has seven fields:

| Field | What to put in it |
| --- | --- |
| **Name** | Any name, except Local in any capitals |
| **Qdrant URL** | The gRPC address of Qdrant, such as `http://localhost:6334`. It is the gRPC port, not 6333 |
| **Qdrant API key** | Only when that Qdrant has an API key set. The box hides what you type |
| **FalkorDB URL** | Such as `falkor://localhost:6379` |
| **FalkorDB password** | Only when that FalkorDB has a password set. The box hides what you type |
| **Graph** | The name of the graph, `quanty` unless you changed it |
| **Content folder** | The absolute path of the folder that holds the converted pages |

- **Local** is shown and cannot be edited in the sheet. It is what `.env`, the environment and the defaults say, so change Local by editing `.env`.
- **Use** connects to both stores, 5 seconds for each, and makes the database the one in use only when both answer. For a saved database the content folder must be there too. When something fails, the sheet says what, and nothing changes: the database in use, the footer and the Library stay as they were. **Use** is off while an ask, an ingest or a dataset build runs, and while the fields do not pass the checks, with the reason when the pointer rests on it.
- **Forget** removes a saved database from the list. It is off for the one in use. **Cancel** closes the sheet and is off while no database is in use.
- Keep a key or a password in its own field. A key that is part of a URL can show up in a message.

The choice is saved as `.quanty-databases.json` in the quanty folder, beside `.env`. It holds the keys as plain text, so only your account can read it, and git ignores it. You can also write it by hand: [reference.md](reference.md#the-file) shows the file, with `"chosen": "Local"` for Local, which is never listed under `saved`.

Every program that reads the stores obeys the choice: the app, `rag-ingest`, `rag-query`, `rag-dataset` and the MCP server. `quanty-mcp` reads it once, when it starts, so restart it after you switch. A file that cannot be read, or a chosen name that is not in the list, stops a program with `could not read the settings`.

**The environment wins.** When any of `QDRANT_URL`, `QDRANT_API_KEY`, `FALKORDB_URL`, `FALKORDB_PASSWORD`, `FALKORDB_GRAPH` or `CONTENT_DIR` is set to a value in the environment, the command line programs and the MCP server ignore the chosen database altogether, and all six settings come from your environment, `.env` and the defaults. A value in `.env` does not count here, only one in the environment. In the app, Local is then treated as chosen. [reference.md](reference.md#the-order-of-sources) has the rule in full, and [what the app does then](reference.md#in-the-desktop-app).

## Where a library lives

A library is three folders under one root, and the root is the quanty folder for Local:

| Folder | What it holds |
| --- | --- |
| `data/qdrant` | The Qdrant store: every vector |
| `data/falkordb` | The FalkorDB store: the graph |
| `content` | The converted pages, the page pictures and the saved insights |

The rest of `data/` (`concept-cache`, `dataset-cache`, `concept-decisions.jsonl`) and `datasets/` are kept answers and built outputs. They stay on the machine that made them, and a library does not need them.

To back a library up, move it or load it, stop the stores, copy the three folders, start the stores, and add the database in the app. No quanty command does it, and none is needed.

**Back it up.** Run this in the quanty folder, with no ingest running. `SAVE` makes FalkorDB write its file now, so the copy is current. Docker Desktop on a Mac needs no `sudo`. On Linux, the store folders can belong to root, so put `sudo` before `tar` and `rsync`.

```bash
docker compose exec falkordb redis-cli SAVE
docker compose stop
tar -czf "quanty-library-$(date +%F).tar.gz" content data/qdrant data/falkordb
docker compose start
```

**Load a backup** on this machine. This replaces the library you have now, so move it out of the way first. Extract in the quanty folder:

```bash
docker compose down
mv content content.old && mv data/qdrant data/qdrant.old && mv data/falkordb data/falkordb.old
tar -xzf quanty-library-2026-10-22.tar.gz
docker compose up -d
```

Local now shows the loaded library. Delete the `.old` folders when you are sure.

**Move it to a new server**, or bring a laptop's library to a server. Set the server up first, as [server.md](server.md) shows, but do not start its stores yet: copy into a root where `data` and `content` are empty or not there. Replace `me@server` and `/srv/quanty` with your login and the root there. `-R` keeps the folder names. The first two lines save and stop the local stores, so that their files are complete:

```bash
docker compose exec falkordb redis-cli SAVE
docker compose stop
rsync -aR content data/qdrant data/falkordb me@server:/srv/quanty/
docker compose start                 # the local stores again, if you still want them
ssh me@server 'cd /srv/quanty && docker compose up -d'
```

**Then add the database in the app.** The stores of a moved library are at a new address, so a database that names the old address finds nothing. Click the footer entry, choose **New…**, fill in the seven fields for the new place (for a server, the addresses are the ones of the tunnel, as [server.md](server.md#4-add-the-database-in-the-app) shows), and press **Use**. Or change the address of the entry you already have. The Library then lists the moved documents.

> [!NOTE]
> The pictures of figures in an answer are looked for at the path they were saved under. That path is the content folder as it was on the machine that ingested, written out in full. So a moved library shows its figure pictures only when its content folder is reached at the same path as before. Pages, formulas, tables and code do not depend on it. A later version may store relative paths.

## Add your first PDF

The library holds media. A media is a book, a paper or another work, with a title, a category, its authors and its tags. Each PDF is one document of a media, and a document can also have tags of its own.

| Document                                                  | Its PDF                                                                                                                 |
| --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| A chapter of a book                                       | Named `chapter-<number>-<name>.pdf`                                                                                     |
| A part of a book that is not a chapter, such as its index | Any name. It takes a title instead: `--title` on the command line, `document_title` over MCP, **Other part** in the app |
| The PDF of a paper or another work, which has a title     | Any name                                                                                                                |

The repository holds a sample of seven pages. Ingest it, then ask a question. The ingest is paid work: see [Costs](#costs).

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --book "Option Volatility and Pricing" \
  --author "Sheldon Natenberg" --tag options \
  samples/chapter-1-sample-pages.pdf

cargo run --release -p rag-retrieval --bin rag-query -- "What is the Black–Scholes formula for a call option?"

# Ask one document alone, by its stored title or its document id
cargo run --release -p rag-retrieval --bin rag-query -- --document "Option Volatility and Pricing, chapter 1: Sample Pages" "Which pages cover volatility?"
```

A 20-page PDF takes about 15 minutes. If it stops, run the same command again and it carries on. On a finished PDF it does nothing and costs nothing, unless the chapter has no insights yet: then it writes only those.

Every chapter or paper that is ingested also gets a file, `insights.json`, in its folder under `content/`. The model of the Ingest choice writes it from the whole converted text: the insights (short paragraphs, each with the pages it rests on), the takeaways (one line each) and a cheat sheet (headed sections, formulas and small tables). A chapter that has the file is not asked again. A part of a book that is not a chapter, such as its index, table of contents, front cover, glossary or preface, gets none, because it has no argument to summarise: the run says so and asks the model nothing. A paper does get them. A picture that stands alone gets none. Chapters that were ingested before this existed get theirs with `rag-ingest insights`; see [the command](reference.md#writing-the-insights-of-stored-chapters).

<details>
<summary>What the ingest prints while it runs</summary>

Its progress goes to standard error, one line for each step. The summary comes last, on standard output.

1. `checking what is already stored`
2. `opening the pdf`
3. `converting 7 of 7 pages (0 already done)`
4. One line for each page as it is cut out, such as `page 3 cut out`
5. One line for each page with what the page cost, such as `page 3 converted ($0.14)`
6. `writing the graph`
7. `embedding <n> items`
8. `storing the items`
9. `writing the insights, takeaways and cheat sheet`
10. `reading the concepts of <n> items`
11. `linking the concepts of <n> items`

Every line, and when it shows, is in [reference.md](reference.md#ingesting-a-pdf).

</details>

The PDF of a paper can have any name, and a paper often has more than one author:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --paper "Hawkes Processes in Finance" \
  --author "First Author" --author "Second Author" --tag hawkes \
  --doc-tag survey \
  path/to/hawkes-notes.pdf
```

A part of a book that is not a chapter, such as its index, takes `--title`, and its PDF can have any name. The document is stored as the title of the book, a comma and the title of the part, here `Option Volatility and Pricing, Index`, and the Library lists it by its own title, after the chapters:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- pdf \
  --book "Option Volatility and Pricing" --title "Index" \
  path/to/book-index.pdf
```

Three flags label what you ingest. All three are optional.

| Flag        | What it labels                                       |
| ----------- | ---------------------------------------------------- |
| `--author`  | An author of the media. Give it once for each author |
| `--tag`     | A tag of the media                                   |
| `--doc-tag` | This PDF only                                        |

`--author` and `--tag` are used only when the library does not have that media yet: change a stored media with `rag-ingest media`. A media that the library already has keeps its own category, whatever `--book`, `--paper` or `--other` says.

## The desktop app

The app is for people. It opens one window with four tabs: **Ask**, **Library**, **Ingest** and **Datasets**.

```bash
cargo run --release -p gui    # builds and opens target/release/quanty
```

The six panels of Ask, every button and every key are in [reference.md](reference.md#the-desktop-app). Not built yet: the notices tray, the help sheet and the health check.

<details>
<summary>Keys</summary>

| Key                                         | What it does                                                    |
| ------------------------------------------- | --------------------------------------------------------------- |
| <kbd>⌘1</kbd> to <kbd>⌘4</kbd>              | Go to **Ask**, **Library**, **Ingest**, **Datasets**            |
| <kbd>/</kbd> or <kbd>⌘K</kbd>               | Write a question (<kbd>Enter</kbd> asks it)                     |
| <kbd>J</kbd>, <kbd>K</kbd>                  | Next and previous result                                        |
| <kbd>⌘.</kbd> or <kbd>Esc</kbd>             | Stop the search or the answer                                   |
| <kbd>Esc</kbd>                              | Put a maximised panel back, when no search or answer is running |
| <kbd>⇧⌘S</kbd>                              | Copy the answer with its citations                              |

</details>

### Ask

- Ask a question in the mode **Answer** or **Results only**, with five filters: **Media**, **Document**, **Authors**, **Tags** and **Category**.
- Read the answer with its citations, the page each citation stands on, the concept graph, the steps of the search, and questions to ask next.
- **Answer** costs one Gemini embedding call and one call to the selected model on your selected agent subscription. **Results only** writes no answer and costs the embedding call alone.
- **Maximise**, at the top right of the page's panel and of the concept graph, gives that panel the whole tab. **Restore**, in the same place, puts every panel back.
- **Share** opens a menu: **Copy as Markdown**, or **Export as PDF…**, **Export as Markdown…** and **Export as HTML…**, which save the written answer, with its formulas, figures, tables, code and sources, as one file that stands alone. An export is free.

### Library

- A shelf on the left with a **Find** box and one row for each media, with how many documents it has. Only the open media shows its documents, so a media and one of its chapters are two clicks away, however large the library is.
- The page of a media shows its title, a badge for its category, its authors and its tags. Its pencil, **✎ Edit**, changes the category, the authors and the tags, as `rag-ingest media` does. The title cannot change, because it names the media's folder.
- The page of a document shows its chapter or its title, its pages and its items, and a strip of views: Overview, Insights, Code, Formulas, Figures, Tables and Concepts. The Overview shows the document's own tags and the buttons: **Copy id** copies its document id, which `rag-ingest tag` and `rag-ingest delete-document` take, **Read** opens it in Ask, **Edit tags** changes its own tags, as `rag-ingest tag` does, and **Delete** deletes it. **Code**, **Formulas**, **Figures** and **Tables** list every item of that kind in the whole document with its page, and **Read on …** opens it in Ask. **Concepts** lists the document's concepts with their definitions. **Insights** shows the saved insights, takeaways and cheat sheet of the document, and **Export** writes them as a PDF, a Markdown file or an HTML file.
- **Delete** on the page of a media deletes the media with every document of it. A sheet asks first, and a delete cannot be undone. It does what `rag-ingest delete-document` and `rag-ingest delete-media` do, and it never removes the PDF you picked from your own disk.
- Every edit and every delete is free. **Delete** is off while a dataset is being built.

### Ingest

- The **Add media** journey. Choose a media in the **Media** list, or choose **Add new media…**, pick its category, type its title, authors and tags, and press **Save media**.
- Press **Choose a PDF**. For a book, **Part of the book** offers **Chapter** and **Other part**. A chapter's number and name are filled in from a file named `chapter-<number>-<name>.pdf`. An other part, such as the index or the glossary, takes a **Title**. **Tags for this PDF** are the document's own tags.
- The check then runs by itself and is free: it says how many pages the PDF has and how many are converted already, or what would stop a start. For a chapter that is stored but has no insights yet, it says that a start pays only for the insights, takeaways and cheat sheet; for a part of a book that is not a chapter it says that a start pays for nothing.
- **Start ingest** is paid work, the same as `rag-ingest pdf`: the words under it name what is paid, the pages, the embedding, the insights, takeaways and cheat sheet, and the concepts. Keep the app open while it runs; if it stops, start the same PDF again and it carries on. **Add another PDF** keeps the media chosen for the next PDF.
- At the end the tab says what became of the insights, takeaways and cheat sheet: written (the Library shows them under **Insights**), saved already, none for a part of a book that is not a chapter, or not written, with the reason and the command `rag-ingest insights --document <id>`, which is paid, and a **Copy command** button. A start of a PDF that is ingested already says the same, and what the insights cost when the agent was asked for them.

<details>
<summary>What the <b>Ingest</b> bar shows while it runs</summary>

The bar shows the stage and how far it has got:

1. "Checking what is already stored" and "Opening the PDF"
2. "Preparing the pages — 7 of 12 ready · about 9%"
3. "Converting the pages — 3 of 12 done · about 36%", with what the pages have cost so far
4. The later stages, such as "Writing the insights, takeaways and cheat sheet", up to "Linking the concepts"

The percent is an estimate: it runs from the first page cut out to the last page saved.

</details>

### Datasets

- Fill in a form: the dataset's name, **Media** (or **All media**), the documents to hold out of training, the rewrites and the questions per section, the most SFT examples, and a dry-run box. The other options keep the defaults of `rag-dataset build`.
- **Start build** is paid work on the model chosen in the top bar, the same as `rag-dataset build`. Tick the dry-run box and it is free: it exports, checks and plans, and asks no model.
- While it runs the bar shows the stage in words with "about N%" and the spend so far. At the end a notice gives the counts and the dataset's folder, and **Try again** or **Build another** goes on from there.

### The footer

Every tab has a strip at the foot of the window with three entries. At the left, one for **Google Gemini**, the embedding key, and one for **Jev**, the converter key. Each entry says what is known about that account, and has a link to the page where you see and top up your credit: **Billing** opens the Google AI Studio billing page, and **Console** opens the Jev console.

At the right end, the third entry is the database in use: **Database: not chosen**, or **Database: <name> · connected**. A click on it opens the sheet that [Choose a database](#choose-a-database) describes. Its states are `not checked`, `checking…`, `connected` and `down`. When the database is down, a short hint stands beside the entry, and the sheet shows the whole reason when you open it. The app checks the database at start, and again each time you press **Use**, never on a timer. Neither a key nor a password is ever shown.

It never shows an amount. Neither provider lets an API key read a balance. Google says that [you can only manage your Prepay credit balance in AI Studio](https://ai.google.dev/gemini-api/docs/billing), and [no method of its API](https://ai.google.dev/api/all-methods) returns one. The [API of Jev](https://api.typesafe.ai/openapi.json) has only `/v1/models` and `/v1/systemone`, and its [error table](https://docs.typesafe.ai/api/errors) has no credit error. So an entry says one of these instead:

| It says | What it means |
| --- | --- |
| balance not offered | The provider took the key. See your credit on the page behind the link |
| key missing | The key is not in `.env`. Add it, then start quanty again |
| key refused | The provider did not take the key. Make a new one, put it in `.env`, then start quanty again |
| out of credit | A reply of that provider said the credit is spent. The next ask (Google) or ingest (Jev) that works clears it |
| could not read | The provider could not be reached, did not answer in time, or answered with an unexpected status. Click the entry to try again |

The footer reads at start, after each ask, ingest and dataset build ends, and when you click an entry. It never reads on a timer. A read costs nothing, and a read that fails or hangs holds nothing else up. The key is never shown.

## Command line

Four programs. Every command and flag is in [reference.md](reference.md).

| Program       | What it does                                                                                                                                                                                                                                                           |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rag-ingest`  | Checks the services, ingests a PDF or a converted document, writes the missing insights of stored chapters (`insights`), changes a media (`media`) or the own tags of a document (`tag`), and deletes a document (`delete-document`) or a whole media (`delete-media`) |
| `rag-query`   | Asks the stored items of the whole library, or of one document (`--document`), a question, and `--answer` writes an answer                                                                                                                                             |
| `ocr`         | Converts the PDF of a book's chapter into saved files under `content/`, and stores nothing                                                                                                                                                                             |
| `rag-dataset` | Builds a training dataset for Gemma 4 from the converted documents; `--dry-run` plans it for free                                                                                                                                                                      |

Run each one through `cargo`:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- …
cargo run --release -p rag-retrieval --bin rag-query -- …
cargo run --release -p ocr -- …
cargo run --release -p rag-dataset -- build --name <name> …
```

The recipe for training on a dataset is in [training.md](training.md).

## Costs

These bill for the work that quanty asks of them.

| What                        | Billed to                                                | When                                                                                          |
| --------------------------- | -------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Gemini embeddings           | Your Gemini API key, by the token                        | Every ingest and every question                                                               |
| Jev                         | Your Jev API key                                         | The pages of a PDF, when they are converted                                                   |
| `claude` (Haiku and Sonnet) | The subscription `claude` is signed in to, never the API | Converting pages, finding concepts, writing the insights of a chapter and writing an answer   |
| A dataset build             | The subscription of the agent of the Datasets choice, Codex or Claude | `rag-dataset build`, `build_dataset` and **Start build** on the Datasets tab, never a dry run |

Every ingest and every question reports the tokens each model used and about what they cost, such as `cost of this run: ≈ $1.75`. The figure comes from one table of prices per million tokens: it is what the API would charge, not what your subscription bills. The table, its date and how each figure is made are in [reference.md](reference.md#costs).

Converted pages and the answers about concepts are kept on disk, so a run that stopped goes on without paying twice, and a PDF that is already ingested costs nothing once its `insights.json` is saved. `rag-ingest insights` is paid work too, for each chapter whose file is missing, unless it is a dry run. The answers of a dataset build are kept in `DATASET_CACHE_DIR`, so a build asked again does not pay twice. `rag-ingest pdf` checks the Gemini key and both stores before the first page is paid for.

These cost nothing:

- The two stores, which run on your machine.
- `health`, `--fixture`, `list_documents`, `read_page`, `chapter_contents`, `chapter_concepts`, `chapter_insights`, `export_insights`, `edit_media`, `edit_document_tags`, `ingest_status`, `insights_status`, `dataset_status`, `stop_job`, `media`, `tag`, `delete-document`, `delete-media` and `insights --dry-run`.
- A dry run of a dataset: `rag-dataset build --dry-run`, `dry_run: true` in `build_dataset`, and the dry-run box of the Datasets tab.
- The check, every edit, every delete and every export of the desktop app.

`delete-document`, `delete-media` and a delete in the desktop app cost nothing to run, but they remove the converted pages, so the same PDF ingested again is converted again, which is paid work.

## Start fresh

To empty the library and begin again, stop the stores and remove the two folders that quanty made on your machine.

> [!CAUTION]
> This deletes every stored media and document, every converted page (with the PDFs that the MCP server saved under `content/_uploads`), every kept answer about concepts, every kept answer of the dataset builds, and the log of concept decisions. It cannot be undone.

```bash
docker compose down       # stops Qdrant and FalkorDB
rm -rf data content       # the stores, the kept answers and the converted documents
docker compose up -d
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

Nothing in git is touched: `samples/` stays. To ingest the PDFs again is paid work again, because nothing is kept to go on from: see [Costs](#costs). The built datasets are in `datasets/`, which `rm -rf data content` leaves: remove it too to drop them. The kept answers of their models are in `data/dataset-cache`, so they go with `data`, and a dataset built again is paid work again. When `.env` names other places with `CONTENT_DIR`, `CONCEPT_CACHE_DIR`, `CONCEPT_DECISION_LOG`, `DATASET_DIR` or `DATASET_CACHE_DIR`, remove those too.

## Artifact descriptions

Converted documents require saved Library artifact titles and descriptions. Existing libraries can preview and backfill them without rebuilding search data:

```sh
rag-ingest artifacts --all --dry-run
rag-ingest artifacts --document <document-id>
```

Backfill uses the selected Ingest model and saved chapter text. See [artifact metadata and backfill](reference.md#artifact-titles-and-descriptions) for resumability, request limits and failure behavior.

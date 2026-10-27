# Using quanty

How to choose a model, find your library, add a PDF, and use the browser app and command line. To install and start quanty, see the [README](../README.md).

- [Choose a model](#choose-a-model)
- [Where a library lives](#where-a-library-lives)
- [Add your first PDF](#add-your-first-pdf)
- [The app in the browser](#the-app-in-the-browser)
- [Command line](#command-line)
- [Costs](#costs)
- [Start fresh](#start-fresh)
- [Artifact descriptions](#artifact-descriptions)

## Choose a model

The **Model** and **Thinking** dropdowns in the top bar are for the tab on screen. Ask,
Ingest and Datasets each keep their own Model and Thinking choice, which the top bar shows on that
tab; Library has none. Ask is for answers, Ingest for page conversion and concept extraction, and
Datasets for dataset builds. The default is **Codex / GPT-6.1 Sol / Medium**. Claude models remain
in the same menu. The controls use the app's existing dropdown styling and save immediately;
finish any running operation before switching.

All language-model calls run headlessly through the installed CLI. On the server, run `codex login --device-auth` once
with your ChatGPT account, or use `claude setup-token` when using Claude; [server.md](server.md#3-model-tools-and-headless-sign-in) explains service account credentials. No OpenAI API key is
needed. Codex CLI 0.162.1 is tested; it must support `--ignore-user-config` and `--ignore-rules`.

The saved choices are `.quanty-model.json` in the server’s quanty home folder. The file holds `ask`,
`ingest` and `datasets`, one choice for each kind of work, each with `provider`, `model` and
`thinking`. The server, command line tools, OCR and MCP server read it before creating a model
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

PDF pages are rendered on the server and attached as images, with their text layer, to headless
Codex. No model shell tools are needed. Gemini embedding and Jev conversion settings keep
their existing roles. Codex usage records tokens; dollar cost is unknown when no price is
available. [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode)
describes the CLI protocol.

## Where a library lives

The server owns one library, named by its `.env`. The browser needs only the server's URL. On the Ubuntu deployment the library is three folders:

| Folder | What it holds |
| --- | --- |
| `/srv/quanty/data/qdrant` | Vectors |
| `/srv/quanty/data/falkordb` | The graph |
| `/srv/quanty/content` | Converted pages, pictures, insights and uploaded source PDFs |

`data/uploads` and `data/exports` stage uploads and downloads. The rest of `data/` holds kept model answers; `datasets/` holds built training data. Those folders stay on the server. Closing a browser tab does not copy or remove them.

Back up the three library folders together after saving Redis and stopping quanty and both stores. Keep settings and credentials securely too. [The server guide](server.md#7-back-up-and-move-a-library) gives the backup and move procedure.

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

## The app in the browser

Open your quanty server's HTTPS URL and sign in with its shared password. For a laptop, start it with:

```bash
cargo run --release -p server -- --open
```

The default loopback address needs no login when no password is configured. The browser app has four tabs: **Ask**, **Library**, **Ingest** and **Datasets**. Its session survives a reload and, with a configured session key, a server restart. The detailed controls are in [reference.md](reference.md#the-server-and-the-app).

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
- **Share** opens a menu: **Copy as Markdown**, or **Export as PDF…**, **Export as Markdown…** and **Export as HTML…**, which download the written answer, with its formulas, figures, tables, code and sources, as one file that stands alone. An export is free.

### Library

- A shelf on the left with a **Find** box and one row for each media, with how many documents it has. Only the open media shows its documents, so a media and one of its chapters are two clicks away, however large the library is.
- The page of a media shows its title, a badge for its category, its authors and its tags. Its pencil, **✎ Edit**, changes the category, the authors and the tags, as `rag-ingest media` does. The title cannot change, because it names the media's folder.
- The page of a document shows its chapter or its title, its pages and its items, and a strip of views: Overview, Insights, Code, Formulas, Figures, Tables and Concepts. The Overview shows the document's own tags and the buttons: **Copy id** copies its document id, which `rag-ingest tag` and `rag-ingest delete-document` take, **Read** opens it in Ask, **Edit tags** changes its own tags, as `rag-ingest tag` does, and **Delete** deletes it. **Code**, **Formulas**, **Figures** and **Tables** list every item of that kind in the whole document with its page, and **Read on …** opens it in Ask. **Concepts** lists the document's concepts with their definitions. **Insights** shows the saved insights, takeaways and cheat sheet of the document, and **Export** writes them as a PDF, a Markdown file or an HTML file.
- **Delete** on the page of a media deletes the media with every document of it. A sheet asks first, and a delete cannot be undone. It does what `rag-ingest delete-document` and `rag-ingest delete-media` do, and it never removes the PDF you picked from your own disk.
- Every edit and every delete is free. **Delete** is off while a dataset is being built.

### Ingest

- The **Add media** journey. Choose a media in the **Media** list, or choose **Add new media…**, pick its category, type its title, authors and tags, and press **Save media**.
- Press **Choose a PDF** to upload a local file to the server. For a book, **Part of the book** offers **Chapter** and **Other part**. A chapter's number and name are filled in from a file named `chapter-<number>-<name>.pdf`. An other part, such as the index or the glossary, takes a **Title**. **Tags for this PDF** are the document's own tags.
- The check then runs by itself and is free: it says how many pages the PDF has and how many are converted already, or what would stop a start. For a chapter that is stored but has no insights yet, it says that a start pays only for the insights, takeaways and cheat sheet; for a part of a book that is not a chapter it says that a start pays for nothing.
- **Start ingest** is paid work, the same as `rag-ingest pdf`: the words under it name what is paid, the pages, the embedding, the insights, takeaways and cheat sheet, and the concepts. Closing the tab leaves an ingest running on the server. After a reload, the app reports that work is running and disables a second start; it does not reattach to progress. If the job fails, choose the same PDF again to continue from saved work. **Add another PDF** keeps the media chosen for the next PDF.
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
- One ingest or dataset build can run at a time across browser tabs. Closing a tab leaves the job running on the server. While connected, its bar shows the stage in words with "about N%" and the spend so far. At the end a notice gives the counts and the dataset's folder, and **Try again** or **Build another** goes on from there.

### The footer

Every tab has a strip at the foot of the window with three entries. At the left, one for **Google Gemini**, the embedding key, and one for **Jev**, the converter key. Each entry has a link to the page where you see and top up your credit: **Google billing** opens the Google AI Studio billing page, and **Jev billing** opens the Jev console. A green dot and the link alone mean the provider took the key. Otherwise the entry says what is wrong.

At the right end, the connection entry says **Connected**, **Reconnecting** or **Lost**. It describes the browser's connection to quanty. After reconnecting, the app checks health and refreshes the catalogue. A command sent while disconnected fails with "not connected to the server"; try it again once connected.

It never shows an amount. Neither provider lets an API key read a balance. Google says that [you can only manage your Prepay credit balance in AI Studio](https://ai.google.dev/gemini-api/docs/billing), and [no method of its API](https://ai.google.dev/api/all-methods) returns one. The [API of Jev](https://api.typesafe.ai/openapi.json) has only `/v1/models` and `/v1/systemone`, and its [error table](https://docs.typesafe.ai/api/errors) has no credit error. So when something is wrong, an entry says one of these instead:

| It says | What it means |
| --- | --- |
| key missing | The key is not in `.env`. Add it, then start quanty again |
| key refused | The provider did not take the key. Make a new one, put it in `.env`, then start quanty again |
| location refused | Google refuses the address the server calls from, not the key. On a host with IPv6, set `EMBEDDING_GEMINI_IPV4_ONLY=true` in `.env` and start quanty again; see [troubleshooting](troubleshooting.md) |

| out of credit | A reply of that provider said the credit is spent. The next ask (Google) or ingest (Jev) that works clears it |
| could not read | The provider could not be reached, did not answer in time, or answered with an unexpected status. Click the entry to try again |

The footer reads at start, after each ask, ingest and dataset build ends, and when you click an entry that says something. It never reads on a timer. A read costs nothing, and a read that fails or hangs holds nothing else up. The key is never shown.

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

- The two stores, which run beside quanty on the server.
- `health`, `--fixture`, `list_documents`, `read_page`, `chapter_contents`, `chapter_concepts`, `chapter_insights`, `export_insights`, `edit_media`, `edit_document_tags`, `ingest_status`, `insights_status`, `dataset_status`, `stop_job`, `media`, `tag`, `delete-document`, `delete-media` and `insights --dry-run`.
- A dry run of a dataset: `rag-dataset build --dry-run`, `dry_run: true` in `build_dataset`, and the dry-run box of the Datasets tab.
- The check, every edit, every delete and every export of the browser app.

`delete-document`, `delete-media` and a delete in the browser app cost nothing to run, but they remove the converted pages, so the same PDF ingested again is converted again, which is paid work.

## Start fresh

On the server, finish running work and take a backup first. Stop quanty and both stores before replacing library folders. This removes the current library from use; ingesting its PDFs again is paid work.

```bash
sudo systemctl stop quanty qdrant redis-server
cd /srv/quanty
stamp=$(date +%F-%H%M%S)
sudo mv content "content.$stamp"
sudo mv data "data.$stamp"
sudo install -d -o quanty -g quanty -m 0750 content data data/qdrant data/uploads data/exports
sudo install -d -o redis -g redis -m 0750 data/falkordb
sudo setfacl -m u:redis:--x /srv/quanty /srv/quanty/data
sudo systemctl start redis-server qdrant quanty
```

The old folders remain beside the new ones until you deliberately delete them. Built datasets remain in `datasets/`. Preserve `.env`, credentials and model choices. If `.env` names other content or cache folders, account for those separately. On a Mac using Docker for its stores, use the Mac quickstart's reset procedure instead.

## Artifact descriptions

Converted documents require saved Library artifact titles and descriptions. Existing libraries can preview and backfill them without rebuilding search data:

```sh
rag-ingest artifacts --all --dry-run
rag-ingest artifacts --document <document-id>
```

Backfill uses the selected Ingest model and saved chapter text. See [artifact metadata and backfill](reference.md#artifact-titles-and-descriptions) for resumability, request limits and failure behavior.

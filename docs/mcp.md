# The MCP server

`quanty-mcp` lets an AI agent use quanty: search the stored library of books, papers and other media, read the pages of its documents, list the formulas, figures, tables, code listings and concepts of a chapter, read its insights and export them to a file, send a PDF to be ingested, write the missing insights of stored chapters, change the labels of a media or a document, stop a job, and build a training dataset from the converted documents. It speaks the [Model Context Protocol](https://modelcontextprotocol.io), through quanty’s authenticated HTTP endpoint, or through `quanty-mcp` over standard input and output on the server itself.

## 1. Before you start it

The running `quanty` server exposes the one library named by its `.env`: Qdrant, FalkorDB, the graph and the content folder. Set it up using [server.md](server.md), including the Gemini and Jev keys, Poppler, and the selected model CLI's sign-in as the service account. Do not set `ANTHROPIC_API_KEY` when using the Claude subscription.

Set `QUANTY_MCP_TOKEN` to a private random token (`openssl rand -hex 32`) in the server's environment file and restart quanty. Every remote call requires `Authorization: Bearer <QUANTY_MCP_TOKEN>`. The browser's password cookie does not authorize MCP. Without a configured token, `/mcp` returns 404; with a missing or wrong bearer token, it returns 401.

The `health` tool checks the stores, model sign-in and keys for this library, and reports the model choices. The environment takes precedence over `.env`; remaining values use the defaults. There is one library per server.

For stdio, start `quanty-mcp` on the server itself with `/srv/quanty` as its working directory. A client started in a different project must supply that working directory, or provide all settings and absolute folder paths in its environment.

## 2. Start it

Remote MCP is part of the same quanty service as the app:

```text
https://quanty.example.com/mcp
Authorization: Bearer YOUR_QUANTY_MCP_TOKEN
```

The endpoint uses streamable HTTP behind Caddy HTTPS. See [section 10](#10-add-it-to-claude-code) for client configuration.

For stdio on the host:

```bash
cargo build --release -p mcp
cd /srv/quanty
/path/to/quanty/target/release/quanty-mcp
```

The client starts that process. Standard output carries only MCP; logs go to standard error. `quanty-mcp` provides stdio only. Local stdio uses its process account and library settings, without the HTTP bearer token.

The unified HTTP server admits one background ingest, insights job or dataset build at a time across MCP and browser tabs. `/api/facts` shows its kind and name while it runs. A separate `quanty-mcp` stdio process keeps one job of each kind at a time; it does not share admission with the HTTP server or other processes. Searches, reads and status calls remain available while a background job runs.

## 3. The tools

Twenty-one tools. A tool that costs money says so in its description, and so does this table.

| Tool | What it does | Costs |
| --- | --- | --- |
| `search` | The stored items (text chunks, formulas, figures, tables, code listings) nearest to a question | A small Gemini call |
| `answer` | An answer written from what `search` finds, with its sources | A Gemini call and your Claude usage; a minute or two |
| `list_documents` | Every stored document with its media, category, authors, tags, chapter, whether it was ingested whole and how many items of each kind it has | Nothing |
| `read_page` | The pieces of one page of a document, in reading order | Nothing, and it needs no store |
| `chapter_contents` | Every formula, figure, table and code listing of a document, in reading order | Nothing, and it needs no store |
| `chapter_concepts` | The concepts of a document, or of one page of it | Nothing; it reads FalkorDB |
| `chapter_insights` | The insights, takeaways and cheat sheet written for a document | Nothing, and it needs no store |
| `export_insights` | Writes the saved insights, takeaways and cheat sheet of a document to a PDF, Markdown or HTML file | Nothing; writes one file |
| `edit_media` | Changes the category, authors or tags of a stored media, in both stores | Nothing |
| `edit_document_tags` | Adds tags to a stored document or takes tags away, in both stores | Nothing |
| `health` | Whether Qdrant, FalkorDB, the sign-in of the Ingest choice's agent and the two keys are ready, and the model of each kind of work | Nothing |
| `ingest_pdf` | Converts one PDF of a media, stores it and writes its insights | **Paid and slow**: Claude, Jev and Gemini; minutes for each PDF |
| `ingest_status` | Deprecated: call `job_status`. How an ingest job is going | Nothing |
| `write_insights` | Writes the missing insights, takeaways and cheat sheets of documents that are stored already | **Paid and slow** unless `dry_run` is `true`: the agent of the Ingest choice is asked once for each document that has none, a minute or more for each. A dry run is free |
| `insights_status` | Deprecated: call `job_status`. How an insights job is going, and what became of each document | Nothing |
| `build_dataset` | Builds a training dataset for Gemma 4 from the converted documents | **Paid and slow** unless `dry_run` is `true`: the agent of the Datasets choice writes every rewritten text and answers every question, which can take hours. A dry run is free |
| `dataset_status` | Deprecated: call `job_status`. How a dataset build is going, and what it spent so far | Nothing |
| `job_status` | A job of any kind by its `job_id`: state, stage with counts and share done, usage and error | Nothing |
| `watch_job` | Waits until the stage or the state of a job changes (up to 90 seconds), then gives what `job_status` gives; sends progress notifications while it waits | Nothing |
| `list_jobs` | The jobs of this server, newest first, and the job that holds the shared background slot | Nothing |
| `stop_job` | Stops a running ingest, insights job or dataset build | Nothing |

`search` and `answer` use the Ask choice, `ingest_pdf` and `write_insights` the Ingest choice and `build_dataset` the Datasets choice. `health` shows all three.

An optional argument can be left out or set to `null`. An optional text that is blank counts as left out.

First calls to try, in this order:

1. `health` with no arguments: `healthy` must be `true`.
2. `list_documents` with no arguments: the documents that are stored, each with its `document_id`.
3. `search` with `{"question": "What is the Black–Scholes formula for a call option?"}`: then give the `document_id` and `page` of a result to `read_page`.

### `search`

| Argument | |
| --- | --- |
| `question` | Required. The question, in plain words |
| `kind` | `chunk`, `formula`, `figure`, `table` or `code` |
| `document` | Only one document: its `title` or its `document_id` from `list_documents`. The title is the whole stored title, whatever the capitals. A title or an id that no stored document has gives no results |
| `media` | Only documents of this media, whatever the capitals. The older name `book` is understood too |
| `author` | Only documents by this author, whatever the capitals. It matches any author of a document |
| `category` | Only documents of this category: `book`, `paper` or `other` |
| `tags` | Only documents that have every one of these tags, each as a tag of the media or as an own tag of the document |
| `limit` | Keep only the first `limit` results (from 1) |
| `explain` | `true` adds `trace`, which says how the search got to the results |

Gives `results`, best first. Each result has `number`, `document_id`, `document` (its title: the book and the chapter, or a title of its own), `media` (the title of its media; a picture that stands alone has none), `category` (left out when there is no media), `authors`, `media_tags` (the tags of the media), `tags` (the document's own tags), `page` (the place of the page in the document, from 1), `printed_page` (the number printed on the page: cite with it), `last_page` and `last_printed_page` (only for a code listing that page breaks cut, which is one result: the place and the printed number of the page it ends on; left out for every other item), `kind`, `label`, `language` (the language of a code listing in lower case, left out when it is not known), `score`, `reason`, `text` and, for a figure, `picture` (a contained picture URL relative to the app base). `reason` is `{"why": "nearest"}`, `{"why": "concept", "concept": …}` or `{"why": "cited", "by": …, "label": …}`. With `explain`, `trace` has `documents_searched`, `seeds`, `question_concepts`, `seed_concepts`, `related_concepts`, `candidates`, `ranked`, `capped` and `kept`. It also gives `usage` and `cost_usd`, as below. Give the `document_id` and `page` of a result to `read_page` to read around it; for a code listing with `last_page`, read each page from `page` to `last_page`.

### `answer`

The arguments `question`, `kind`, `document`, `media`, `author`, `category` and `tags` of `search`. Gives `answered` (false when nothing was found, and then Claude is not asked, or when what was found does not answer the question), `title`, `claims` (each with `heading`, `text` and the `sources` it rests on, by number), `sources` (each item that a claim names, once: `number` and the fields of a search result without `score` and `reason`) and `follow_ups`. The sign-in of `claude` and `ANTHROPIC_API_KEY` are checked before the search, which costs nothing, so nothing is embedded when `claude` cannot answer. An agent that can write its own answer should call `search`.

`search` and `answer` also give what the call used. `usage` has one entry for each model: `model`, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens` and `estimated` (true for the embedding, whose count is worked out from the length of the text). `cost_usd` is about what the call cost in US dollars at the prices of the API, or `null` when a model has no price; the table and its date are under [Costs](reference.md#costs). `search` uses the embedding alone. `answer` adds the model that wrote it, and when nothing was found it gives what the search used.

```json
"usage": [{"model": "gemini-embedding-2", "input_tokens": 12, "output_tokens": 0, "cache_read_tokens": 0, "cache_write_tokens": 0, "estimated": true}],
"cost_usd": 0.0000024
```

### `list_documents`

No arguments. Gives `documents`, sorted by title: `document_id`, `title` (the whole stored title), `media` and `category` (left out for a picture that stands alone, which belongs to no media), `authors`, `media_tags` (the tags of the media), `tags` (the document's own tags) and, when the converted document is under the content folder, `pages`, with `chapter_number` and `chapter_name` for a book's chapter. It also gives `ingested_whole` and `items`. `items` says how many items of each kind the stores hold for the document: `chunk`, `formula`, `figure`, `table` and `code`. `ingested_whole` is `false` when an ingest stopped or skipped an item, or when the document was stored before quanty kept this mark: a search may miss part of it, and sending its PDF again with `ingest_pdf` finishes it. Give the `title` or the `document_id` of one as `document` to `search` or `answer` to ask that document alone.

It also gives `unreadable_chapters`: one line for each converted document under the content folder whose `chapter.json` cannot be read, with the file that is at fault. The list is always there, and it is empty when every one can be read. Such a document shows no `chapter_number`, `chapter_name` or `pages`, and `read_page` cannot read it.

### `read_page`

| Argument | |
| --- | --- |
| `document_id` | Required. From a search result or `list_documents` |
| `page` | Required. The `page` of a search result (from 1), not the printed number |

Gives `document_id`, `media` (the title of its media), `chapter_number` and `chapter_name` (for a book's chapter), `document_title` (for a document with a title of its own, such as a paper's or a part of a book that is not a chapter; it gives the whole stored title, which for a part is "<book>, <part>"), `page`, `pages`, `printed_page`, `page_image` (the contained whole-page picture URL relative to the app base) and `pieces`: for each, `number`, `kind` (`heading`, `text`, `formula`, `figure`, `table`, `footnote` or `code`), `section` (the headings it sits under, outermost first), `label`, `caption` (the caption as printed, without its label, for a figure, table or code listing that has one), `name` (the name the book gives a formula, such as "Black–Scholes call price", when it names it), `language` (for code, when it is known), `content` (the code as printed, with its indentation, for code) and, for a figure, `picture`.

### `chapter_contents`

| Argument | |
| --- | --- |
| `document_id` | Required. From `list_documents` or a search result |
| `kind` | `formula`, `figure`, `table` or `code`: only items of that kind. Every kind when left out. `chunk` is refused: read the text of a page with `read_page` |

Lists every formula, figure, table and code listing of one converted document, in reading order. It reads the saved chapter, so it needs no store and shows while the stores are down. Gives the same `document_id`, `media`, `chapter_number`, `chapter_name`, `document_title` and `pages` as `read_page`, then `counts` (`formula`, `figure`, `table` and `code`, always of the whole document, whatever `kind` asked) and `items`. Each item has `page` (the place of the page it starts on, from 1: give it to `read_page` to read around the item), `printed_page`, `number`, `kind`, `section`, `label`, `caption` (a figure, table or code listing), `name` (a formula), `language` (code), `content` (the LaTeX of a formula, the code as printed, the explanation of a figure, the Markdown of a table) and, for a figure, `picture`. A code listing that page breaks cut is one item: `content` holds all of it, and `last_page` and `last_printed_page` say where it ends.

### `chapter_concepts`

| Argument | |
| --- | --- |
| `document_id` | Required. From `list_documents` or a search result |
| `page` | Only the concepts of this page: the `page` of a search result or of `chapter_contents` (from 1), not the printed number. The whole document when left out |

Gives `document_id`, `title` (the whole stored title), `page` (only when asked) and `concepts`, the most mentioned first: each has `name` and a one-line `definition`. A page with no concept gives an empty list. It reads FalkorDB, as the browser app does beside a page.

### `chapter_insights`

| Argument | |
| --- | --- |
| `document_id` | Required. From `list_documents` or a search result |

Gives what the agent of the Ingest choice wrote about the whole document when it was ingested, from the file `insights.json` in its converted folder: the same `document_id`, `media`, `chapter_number`, `chapter_name`, `document_title` and `pages` as `read_page`, and `state`:

| `state` | Meaning |
| --- | --- |
| `saved` | The notes are there: `file`, `model` (the model that wrote them), `prompt_version`, `insights` (each a short Markdown paragraph with `text` and the `pages` it rests on, counted from 1: give one to `read_page`), `takeaways` (one line of Markdown each) and `cheat_sheet` (Markdown with short headed sections, formulas and small tables) |
| `not_written_yet` | Nobody has written them. Call `write_insights` with this `document_id`, which is paid. `insights` and `takeaways` are empty |
| `none_for_book_part` | A part of a book that is not a chapter, such as its index, gets none |

It costs nothing. It reads the file, and it reads the graph only for a document with a title of its own that has no file, to see whether it is a part of a book. A file that cannot be read is a tool error that names it and says to call `write_insights`, which writes it again.

### `export_insights`

| Argument | |
| --- | --- |
| `document_id` | Required. From `list_documents` or a search result |
| `format` | Required. `pdf`, `markdown` or `html`, whatever the capitals |
| `path` | Required. The absolute path of the file to write, on the machine the server runs on, ending in `.pdf`, `.md` or `.html` as `format` is. Its folder must exist |
| `overwrite` | `true` to replace a file that is at `path`. `false` when left out: a file that is there is kept and the call fails |

Writes what `chapter_insights` reads to one file that stands alone, the same file that **Export** in the Insights view of the browser app writes: the title of the document, a line that names the model, the insights with their pages, the takeaways and the cheat sheet. The PDF has its formulas typeset, the HTML file has its pictures inside it, and the Markdown file keeps its formulas as `$…$`. Every check runs before anything is written, in this order: `format`, that `path` is absolute, that it ends with the extension of the format (whatever the capitals), that its folder is there, that no file is at `path` unless `overwrite` is `true` (a link that points nowhere counts as a file), and then the saved insights, which it reads as `chapter_insights` does. A document with no saved insights is refused with a text that says to call `write_insights`, which is paid, and so is a part of a book that is not a chapter, which gets none. It makes no folder. Gives `document_id`, `format`, `path` (as it was given) and `bytes` (the size of the file). It costs nothing: it asks no model and embeds nothing. The tool says `destructive` to the client because `overwrite` can replace a file.

### `edit_media`

| Argument | |
| --- | --- |
| `media` | Required. The title of the stored media, whatever the capitals, as `list_documents` gives it |
| `category` | `book`, `paper` or `other`. A book's chapter PDFs must be named `chapter-<number>-<name>.pdf` |
| `authors` | The authors, in order. They replace the old ones; `[]` takes every author away |
| `tags` | The tags of the media. They replace the old ones; `[]` takes every tag away |

A label that is given replaces the old one, and one that is left out stays. Give at least one. Every document of the media, and every stored item of those documents, gets the new labels in both stores, so `search` and `answer` filter by them at once. The title names the media and cannot change. Nothing is embedded and no model is asked, so it costs nothing, and the old values give the labels back. It does what `rag-ingest media` does. Gives `media` (as the library has it), `category`, `authors`, `tags` and `documents` (how many documents now carry them). The arguments are checked before any store is reached, so a bad edit changes nothing. The tool says `destructive` to the client because a given list replaces the old one.

### `edit_document_tags`

| Argument | |
| --- | --- |
| `document_id` | Required. From `list_documents` or a search result |
| `add` | Tags to give the document |
| `remove` | Tags to take away, after `add` |

Changes the own tags of one stored document in both stores. The tags of its media stay: change those with `edit_media`. Adding a tag the document has, or taking away one it lacks, changes nothing. Give at least one tag. Gives `document_id`, `tags` (the own tags after the change) and `media_tags`. It costs nothing, like `edit_media`, and does what `rag-ingest tag` does.

### `health`

No arguments. It runs every check, so one report names everything that is wrong, and it costs nothing. Gives:

- `healthy`: `true` when Qdrant, FalkorDB and the agent of the Ingest choice are ready and both keys are `accepted`. A key that could not be checked counts as not ready.
- `report`: one line for each of Qdrant, FalkorDB and the agent of the Ingest choice, then `Gemini key` and `Jev key`: `ok (accepted)`, or `FAILED (<state>)` and why.
- `keys`: the Google Gemini key (`EMBEDDING_GEMINI_API_KEY`) and then the Jev key (`CONVERTER_JEV_API_KEY`). Each has `key`, `service`, `state`, `detail` and `billing_page`. Each key is checked with the one free call its provider allows, both at once and for at most 5 seconds. Neither Google nor Jev gives a balance to a key, so the credit is seen on `billing_page` (`https://aistudio.google.com/billing` for Gemini, `https://console.typesafe.ai/` for Jev). Spent credit shows only when the provider says so in answer to a paid call; the free call cannot see it, so a key can be `accepted` and out of credit.
- `models`: the model and thinking that `ask`, `ingest` and `datasets` use, as saved (`provider`, `model` and `thinking`), with the `tools` that spend each choice. A choice that cannot be read has an `error` instead.

| `state` of a key | Meaning |
| --- | --- |
| `accepted` | The provider accepted the key |
| `missing` | The settings hold no key. Add it to `.env` or to the environment and start the server again |
| `refused` | The provider does not accept the key. Put a good key in `.env` and start the server again |
| `out_of_credit` | The provider says the prepaid credit is used up. Add credit on `billing_page` |
| `unread` | The check could not tell: the provider could not be reached, gave no answer in 5 seconds, or answered with an HTTP status that does not mean a refusal. `detail` says which |

The keys are read when the server starts, so a key that is fixed in `.env` needs a restart. No tool shows a key.

### `ingest_pdf` and `ingest_status`

See the next part. `ingest_status` is deprecated: `job_status` gives the same result.

### `write_insights` and `insights_status`

See part 6. `insights_status` is deprecated: `job_status` gives the same result.

### `build_dataset` and `dataset_status`

See part 5. `dataset_status` is deprecated: `job_status` gives the same result.

### `job_status`, `watch_job`, `list_jobs` and `stop_job`

See part 7.

## 4. Sending a PDF

`ingest_pdf` takes the media, the PDF and labels:

| Argument | |
| --- | --- |
| `media` | Required. The title of the media the PDF belongs to, such as "Option Volatility and Pricing". The older name `book` is understood too |
| `category` | `book` (the default), `paper` or `other`. Used only when the library does not have this media yet: a stored media keeps its own category, which says how the PDF is named |
| `authors` | The authors of the media, in order. Used only when the library does not have this media yet |
| `tags` | The tags of the media. Used only when the library does not have this media yet |
| `document_tags` | The tags of this PDF only. They are added to the tags that the document has |
| `document_title` | The title of the document. For a paper or another media it defaults to the title of the media. For a book it names a part that is not a chapter, such as "Index" or "Glossary": the document is stored as "<book>, <title>" and the file can have any name. Leave it out for a book's chapter, whose file is named `chapter-<number>-<name>.pdf` |

An ingest never changes a stored media: to change its labels, use `edit_media` (or `edit_document_tags` for the tags of one document), `rag-ingest media`, or the Library tab of the browser app. The media is found by its title whatever its capitals and the space at its ends.

The PDF is sent one of two ways. Give exactly one of them.

| Way | Arguments | For |
| --- | --- | --- |
| By path | `path`: the absolute path of the file on the machine the server runs on | An agent on the same machine. This is the way to use |
| As base64 | `pdf_base64`: the bytes as standard base64, and `file_name` | A program client. An AI agent cannot write megabytes of base64 itself |

Rules:

- The media is a book when the library has it as a book, and a new media is a book when `category` is left out or is `book`. The PDF of a book's chapter is named `chapter-<number>-<name>.pdf`, such as `chapter-1-financial-contracts.pdf`: the chapter comes from the file name. A book's file with another name and no `document_title` must be copied or renamed first, or sent with a `document_title`.
- A part of a book that is not a chapter, such as its index, takes a `document_title` and can have any file name. It is stored as "<book>, <title>", such as "Option Volatility and Pricing, Index".
- The PDF of a paper or another media can have any name.
- It must be an existing file that starts with `%PDF-` and is at most 50 MiB.
- An upload (`pdf_base64`) is saved under the content folder, in `_uploads/<media folder>/<file name>`, and never at a path that the caller chooses. A `file_name` with a folder in it is refused.
- The converted pages are saved under the content folder, in `<media folder>/chapter-<number>/` for a book's chapter and in `<media folder>/<document title folder>/` for any other document, which for a part of a book is `<media folder>/<part title folder>/`, made from the title of the part alone. The folders are made from the titles: lower case, with one dash for each run of characters that are not letters or digits.
- The ingest also writes `insights.json` in that folder: the insights, the takeaways and the cheat sheet that the agent of the Ingest choice wrote about the whole document, from its converted text, in one question. A document whose file is saved is not asked about again. A part of a book that is not a chapter (its index, table of contents, front cover, glossary or preface) gets no insights and no file, and the agent is not asked; a chapter, and a document of another media such as a paper, does get them. Its fields are in the reference, under "Ingesting a converted document".

The call does not wait for the whole ingest, which takes minutes. It answers once the paid work has begun, with the job (part 7): `job_id`, `kind: "ingest"`, `state: "running"`, the `stage` and the times. Then the agent calls `watch_job` with that `job_id`, which waits until the stage changes, or `job_status`, until `state` is not `running`. `ingest_pdf`, `job_status` and `watch_job` give the same result: the fields of the job (part 7) and, beside them, `media` (the title of the media), `file` (the file name), `artifacts_done` and `artifacts_total` (the artifact descriptions written, while they are) and, by `state`:

| `state` | Meaning |
| --- | --- |
| `running` | Going on. `stage` is one of the stages below. With no `stage` the first step has not come yet |
| `done` | Finished. `document_id`, `items`, `summary` (what `rag-ingest pdf` prints), and `usage` and `cost_usd` (what the whole ingest used, in the shape that `search` gives them) are there |
| `already_ingested` | Both stores already held this PDF, so nothing was converted, embedded or stored. `document_id` and `items` are there. The `document_tags` that were sent are still written. When the document's `insights.json` was not saved yet, the agent was asked for it: `summary` (with an `insights:` line that says whether the file was written, and why not when it was not) and `usage` and `cost_usd` are there too, and the job was `running` with the stage `writing_insights` while the agent was asked. Otherwise it cost nothing, a part of a book that is not a chapter included, which gets no insights |
| `failed` | Stopped. `error` says why. Send the same PDF again to go on: converted pages and kept answers are not paid for twice |
| `stopped` | `stop_job` stopped it. `error` says so and ends the same way: send the same PDF again to go on |

The stages of an ingest are those of the progress bar of the browser app, with the same counts and the same `share`:

| `stage.name` | `done` and `total` | What is going on |
| --- | --- | --- |
| `checking_stored` | none | The stores are asked whether the document is stored already |
| `opening_pdf` | none | The PDF is opened and its pages are counted |
| `preparing_pages` | pages ready, pages | The pages are cut out, all before the first one is converted |
| `converting` | pages saved, pages | The pages are read by the agent and Jev: the slow part |
| `writing_graph` | none | The document and its items are written to the graph |
| `embedding` | none, items | The items are embedded |
| `storing` | none | The points are stored |
| `writing_insights` | none | The agent writes the document's insights, takeaways and cheat sheet |
| `writing_artifacts` | descriptions written, items | The artifact descriptions are written |
| `reading_concepts` | items read, items | The items are read for their concepts |
| `linking_concepts` | items linked, items | The concepts are written to the graph |

Good to know:

- Over unified HTTP, one background job runs at a time across MCP and browser tabs; a running ingest, insights job or dataset build refuses another start. Over stdio, one ingest runs at a time, and the refusal names its `job_id`. Do not send the same PDF through two processes, or beside `rag-ingest pdf`, at the same time.
- The graph is read before the PDF is named, because a stored media's category says how the PDF is named. So a store that is down shows before a file name that breaks the rule for a book, as a tool error whose text names the store, says to call `health`, and says to send the same PDF again.
- A store that is down, a missing Gemini key, and a PDF that is already ingested with its insights saved, or that gets none, show in the answer of `ingest_pdf` itself. A PDF that is already ingested without its insights is answered as `running` with the stage `writing_insights`, and its end is read with `watch_job` or `job_status`. A missing Jev key, `ANTHROPIC_API_KEY` being set, Poppler not installed, and a file that starts with `%PDF-` but is not a PDF are found only after the paid work has begun. So they show in `job_status` as `failed`, or, when they are found at once, in the answer of `ingest_pdf` as a tool error with the same text.
- A PDF that was ingested before insights were written has no `insights.json`. Sending it again writes that file and nothing else, unless it is a part of a book that is not a chapter, which gets none. To write the files of many stored chapters at once, without sending their PDFs, call `write_insights` (part 6) or run `rag-ingest insights --all`.
- Jobs live in the memory of the server. Over stdio a running job stops when the client closes the server, and a restarted server forgets its job ids. The server keeps only its last 100 jobs: when it holds 100 and a new one starts, the oldest, which has ended, is forgotten. Send the PDF again: a PDF that is ingested is not ingested twice, and a stopped one goes on from where it ended.

## 5. Building a training dataset

`build_dataset` builds a training dataset for Gemma 4 from the documents that are converted under the content folder: whole book sections for continued pre-training, rewritten texts, checked questions and answers for fine-tuning, and a manifest that says what the dataset holds. It is the same build as `rag-dataset build`. Run a dry run first: `dry_run: true` exports, checks and plans, asks no model, costs nothing, and its `summary` says how many texts, questions and model calls the full build would make.

| Argument | |
| --- | --- |
| `name` | Required. The name of the dataset: letters, digits, `-`, `_` and `.`, and not `.` or `..`. It is the name of its folder under the dataset folder. A build with the name of an earlier one replaces that dataset's files |
| `media` | Only the documents of these media, each a title whatever the capitals. Every media when left out |
| `documents` | Only these documents, each a `document_id` or the whole `title` from `list_documents`. Every document of the chosen media when left out |
| `holdout` | Documents kept out of training for evaluation, named like `documents`. Their sections go to `cpt/heldout.jsonl`, and no synthetic text is written from them |
| `include_book_parts` | Also export the parts of a book that are not chapters, such as its index. An appendix is always exported. `false` when left out |
| `rewrites` | How many rewritten texts are written from each passage of a training section, a part of it of at most 2,500 estimated tokens. 3 when left out |
| `questions` | How many questions each section gets. 2 when left out |
| `max_sft` | The most fine-tuning examples the build writes. 300 when left out |
| `max_synthetic` | The most synthetic texts the build writes. No limit when left out |
| `samples` | How many times the model answers each question, to check that its answers agree. 3 when left out |
| `run_code` | Run the Python that the model writes for an answer, to check it. `true` when left out |
| `dry_run` | Export, check and plan only. `false` when left out |
| `min_page_score` | A section whose worst page matches less of the PDF's own text than this share, from 0 to 1, is rejected. 0.90 when left out |
| `keep_unrendered` | Keep a section whose formulas do not render, instead of rejecting it. `false` when left out |

In `media`, `documents` and `holdout`, each entry has its spaces at both ends cut off, and a blank entry is left out.

The call does not wait for the whole build. It answers once the build has begun, with the job (part 7): `job_id`, `kind: "dataset"` and `state: "running"`, or `done` when the build ended before the call answered. A build that failed by then, such as one that a bad argument stopped before its first step, is a tool error. Then the agent calls `watch_job` with that `job_id`, which waits until the stage changes, or `job_status`, until `state` is not `running`. `build_dataset`, `job_status` and `watch_job` give the same result: the fields of the job (part 7), with `usage` and `cost_usd` (what the models used so far, in the shape that `search` gives them; `cost_usd` is `null` when a model has no price, as Codex has none), and, beside them, `folder` (the dataset's folder, as the settings give it), and, by `state`:

| `state` | Meaning |
| --- | --- |
| `running` | Going on. `stage` is one of the stages below, with no `stage` before the first step has come. `usage` and `cost_usd` are what the models used until the last step |
| `done` | Finished. `summary` says what the build wrote, and `usage` and `cost_usd` are what the whole build used |
| `failed` | Stopped. `error` says why and what to do. `usage` and `cost_usd` are what was used until the last step before it stopped. The answers a model already gave are kept, so a build that is called again is not paid twice for them |
| `stopped` | `stop_job` stopped it. `error` says so and what to do: call `build_dataset` again. `usage` and `cost_usd` are what was used until then, and the answers a model already gave are kept |

| `stage.name` | `words` | `done` and `total` |
| --- | --- | --- |
| `exporting` | `exporting 3 of 12 documents` | documents exported, documents |
| `checking` | `checking the sections` | none |
| `planning` | `planning the generation` | none |
| `writing_synthetic` | `writing 40 of 120 synthetic texts` | texts written, texts planned |
| `writing_sft` | `writing 5 of 300 SFT examples` | examples written, examples planned |
| `writing_manifest` | `writing the manifest` | none |

`summary` is what `rag-dataset build` prints: the dataset and its folder, the documents and the sections for training, held out and rejected (with the reason of each rejection), the texts and questions that were planned, then either `planned model calls` and `dry run: no model was asked`, or the synthetic texts and the SFT examples that were written, then one usage line for each model and the cost of the run.

The build writes these files in the folder `<DATASET_DIR>/<name>` (`DATASET_DIR` defaults to `datasets/`): `cpt/raw.jsonl`, `cpt/heldout.jsonl`, `cpt/rejected.jsonl`, `cpt/synthetic.jsonl`, `sft/train.jsonl`, `sft/heldout.jsonl`, `sft/rejected.jsonl`, `plan.json` and `manifest.json`. A dry run writes only the three files of the sections (`cpt/raw.jsonl`, `cpt/heldout.jsonl` and `cpt/rejected.jsonl`), `plan.json` and `manifest.json`. The answers of the models are kept in `DATASET_CACHE_DIR` (default `data/dataset-cache`), so the same question is never paid for twice. How to train Gemma 4 on the dataset is in [training.md](training.md).

Good to know:

- Over unified HTTP, one background job runs at a time across MCP and browser tabs. A second start is refused while an ingest, insights job or dataset build runs. Over stdio, one build runs at a time and an ingest or insights job may run beside it. Other processes, including `rag-dataset build`, do not share this admission.
- A build with the name of an earlier one removes the files that the earlier build wrote, once it knows it can run, and then writes its own. Give a new `name` to keep the earlier dataset.
- `run_code` is on by default. It runs Python that the model wrote, on the machine the server runs on, to check an answer. `run_code: false` turns that off, and such answers are then checked by the judge alone.
- The graph is connected first, and the model's sign-in is checked before the job starts, unless `dry_run` is `true`: a dry run needs no model. So a graph that is down, or a model that is not signed in, shows in the answer of `build_dataset` as a tool error that ends with: call the `health` tool.
- A blank `name` is refused before anything else. A `name` that cannot name a folder, a `media`, `documents` or `holdout` entry that no converted document matches, and a selection that leaves no document are found by the build before its first step, so the graph must be up to find them. They show in the answer of `build_dataset` as a tool error that says what to give instead.
- Jobs live in the memory of the server. Over stdio a running build stops when the client closes the server, and a restarted server forgets its job ids. The server keeps only its last 100 builds. Call `build_dataset` again: the answers a model already gave are kept.

## 6. Writing the insights of stored chapters

A chapter that was ingested before insights were written has no `insights.json`, so `chapter_insights` says `not_written_yet` for it. `write_insights` writes the missing files of documents that are stored already, from their converted text, with nothing embedded. It is the same work as `rag-ingest insights`. It is **paid** unless `dry_run` is `true`: the agent of the Ingest choice is asked once for each document that has none, which spends its subscription usage and takes a minute or more for each. Run a dry run first: it asks no model, costs nothing and lists what would be written.

| Argument | |
| --- | --- |
| `document_id` | One stored document: its `document_id` from `list_documents` |
| `media` | Every document of this media, by title whatever the capitals |
| `all` | `true` for every stored document that belongs to a media |
| `dry_run` | List what would be written and ask nothing. `false` when left out |

Give exactly one of `document_id`, `media` and `all: true`. A document whose insights are saved is never asked about again, and a part of a book that is not a chapter gets none. The call does not wait for the work. It answers once the first question is out, or the job has ended, with the job (part 7): `job_id`, `kind: "insights"` and `state`. Then the agent calls `watch_job` with that `job_id`, which waits until the stage or the state changes, or `job_status`, until `state` is not `running`. `write_insights`, `job_status` and `watch_job` give the same result: the fields of the job (part 7), with `usage` and `cost_usd` (what the questions used so far, in the shape that `search` gives them; empty and `0` for a dry run or a job that asked nothing) and, beside them, `dry_run`, `done` and `total` (documents handled and chosen), `current` (the title of the document being asked about, only while a question is out) and `documents`. The one `stage` is `writing_insights`, with `done` and `total` the documents handled and chosen, and words that name the document being asked about while a question is out.

| `state` | Meaning |
| --- | --- |
| `running` | Going on |
| `done` | Every chosen document was handled. A document that is `not_written` does not fail the job: its `reason` says why, and calling `write_insights` again tries it once more |
| `failed` | The job stopped. `error` says why and what to do |
| `stopped` | `stop_job` stopped it. What was written until then is kept: call `write_insights` again, and the insights that are saved are not asked for twice |

`documents` has a line for each document handled so far, with `document_id`, `title` and `outcome`:

| `outcome` | Meaning |
| --- | --- |
| `would_write` | A dry run: the insights would be written to `file`, and nothing was asked |
| `written` | The insights were written to `file` |
| `already_saved` | The file was there already, so nothing was asked |
| `not_written` | Nothing was saved, and `reason` says why |
| `none_for_book_part` | A part of a book that is not a chapter gets none |
| `no_converted_chapter` | The graph holds the document, but the content folder holds no converted chapter of it. `reason` names the folder |

Good to know:

- Over unified HTTP, an insights job excludes other background work in MCP and browser tabs. Over stdio, one insights job runs at a time and an ingest or dataset build may run beside it. Other processes do not share this admission.
- The graph is connected first and the documents are chosen before anything is asked. A choice that is not valid, a document or media that is not stored, and a graph that is down show in the answer of `write_insights` as a tool error. Unless `dry_run` is `true`, the sign-in of the agent of the Ingest choice is checked once, which costs nothing, so an agent that is not signed in fails the call before the first document. A job with nothing to write never starts the agent.
- A separate process that ingests the same chapter at the same moment may ask for its insights too. Keep that process idle until the HTTP or stdio job ends.
- Jobs live in the memory of the server. Over stdio a running job stops when the client closes the server, and a restarted server forgets its job ids. The server keeps only its last 100 insights jobs. Call `write_insights` again: insights that are saved are not asked for twice.

## 7. Following and stopping a job

`ingest_pdf`, `write_insights` and `build_dataset` start a job and answer at once. Every tool that is about a job gives the same view of it, whatever its kind, and `job_status`, `watch_job` and `list_jobs` follow jobs of any kind by their `job_id`.

### The job

The result of a job tool is these fields at the top level, with the fields of its kind (parts 4 to 6) beside them:

| Field | |
| --- | --- |
| `job_id` | Give it to `job_status`, `watch_job` and `stop_job` |
| `kind` | `ingest`, `insights` or `dataset` |
| `state` | `running`, `done`, `already_ingested` (an ingest only), `failed` or `stopped` |
| `stage` | What a running job is doing. Left out when the job is not running, and before its first step |
| `started_at` | When the job was made, as RFC 3339 text in UTC |
| `updated_at` | When its report last changed |
| `usage`, `cost_usd` | What the models used so far, and what the whole job used when it has ended, in the shape that `search` gives them. Left out when the job has nothing to report, such as an ingest that found its PDF stored |
| `error` | Why the job stopped and what to do. Only when `failed` or `stopped` |

`stage` is `{ "name", "words", "done", "total", "share" }`. `name` is a word that does not change between versions, such as `converting`. `words` is a few lower-case words for a person, such as `converting the pages: 3 of 7 done`. `done` and `total` are whole numbers, and `null` for a stage that does not count. `share` is how much of the stage is done, from 0 to 1 to three decimals, and `null` for a stage that does not count. For an ingest the stage names, the counts and the `share` are those of the progress bar of the browser app. For an insights job and a dataset build `share` is `done` over `total`.

### `job_status`

`job_status({ "job_id" })` gives the whole result of the job, whichever kind it is: the fields above and the fields of its kind. A `job_id` that this server does not hold is a tool error: job ids are forgotten when the server restarts, and the server keeps only its last 100 jobs of each kind. It costs nothing.

### `watch_job`

`watch_job({ "job_id", "timeout_s" })` waits and then gives the same result as `job_status`. `timeout_s` is a whole number of seconds from 1 to 90, and 60 when left out; any other value is a tool error. It returns:

- at once, when the job is not running;
- on the first change of `state` or of the name of the `stage` since the call began, which includes the first stage of a job that had none;
- at the timeout, with the job as it is. A change of the counts or of the words inside one stage does not end the wait.

When the call carries a progress token, it also sends `notifications/progress` while it waits: one at once, then one every 2 seconds at most, and only when `progress` or `message` changed. `progress` is the `share` of the stage in percent (the last value sent when the stage has no share, and 0 at first), `total` is 100 and `message` is the `words` of the stage. They stop when the call returns. A client that cancels the call ends the wait and nothing else: the job goes on, and the shared background slot is not touched. It costs nothing.

### `list_jobs`

`list_jobs` gives `{ "running", "jobs" }`. `running` is the work that holds the shared background slot of the unified HTTP server, as `{ "kind", "name" }`, whether this server or the browser started it, and `null` when the slot is free and always over stdio, which has no shared slot. `jobs` are the jobs of this server of every kind, newest first, at most the last 20 of each kind, each as the fields above and without the fields of its kind. It costs nothing.

### The older status tools

`ingest_status`, `insights_status` and `dataset_status` still answer, with the same result as `job_status` for a job of their kind. Their descriptions start with "Deprecated: call `job_status`", and they will go once the programs that call them have moved.

### Marks for programs that list tools

Each tool carries `_meta` for programs that list tools: `quanty/group` (`jobs`, `ingest` or `datasets` for the tools of this part and of parts 4 to 6) and `quanty/cost` (`{ "paid": false }`, or `{ "paid": true, "who": "ingest" }` for `ingest_pdf` and `write_insights` and `"datasets"` for `build_dataset`). `ingest_pdf`, `write_insights`, `build_dataset`, `job_status` and `watch_job` also carry `jet/job`: `{ "version": 1, "statusTool": "job_status", "idField": "job_id" }`, so that a host can follow the job with the free status tool after the model's turn has ended. These tools declare an `outputSchema` whose root is an object with the fields above.

### `stop_job`

`stop_job` stops a running job of this server by the `job_id` that started it: an ingest (`ingest_pdf`), an insights job (`write_insights`) or a dataset build (`build_dataset`). A build can take hours, and a stopped job stops spending. It costs nothing.

| Argument | |
| --- | --- |
| `job_id` | Required. The `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave |

Gives `job_id`, `kind` (`ingest`, `insights` or `dataset`), `state` and `status_tool` (the tool that says how the job ended). `state` is `stopped` when the job was running and is stopped now, and `ended` when it had ended already: it is left as it is, and its status tool has how it ended.

What was paid for is kept, so the same work started again goes on from there: converted pages and kept answers of an ingest, the answers a model already gave in a dataset build, and the insights that are saved. The status tool of a stopped job gives `state: "stopped"` and an `error` that says what to call again. This is what the Stop button of the browser app does. A new job can start after the stopped work has ended.

## 8. What is not a tool

The tools do what the browser app does where an agent needs it. These are left out on purpose.

| Not a tool | Why |
| --- | --- |
| Deleting a document or a whole media | A delete removes converted pages, points and graph nodes and cannot be undone, and making them again is paid work. An agent never needs it to read or learn, and a stored page can hold words that lead an agent to call such a tool. A required `confirm` field guards nothing, because a misled agent fills it too. Delete in the Library tab, or with `rag-ingest delete-document` and `rag-ingest delete-media` |
| Changing the model or thinking of a kind of work | It decides what the browser app, the command line and this server spend on every paid call. The person sets it in the top bar or in `.quanty-model.json`. `health` shows it, and an agent needs no other model to do its work |
| Exporting an answer to PDF, Markdown or HTML | The agent already holds the answer, its claims and its sources as JSON. The browser app asks quanty to export the answer it holds, and a tool would need the server to keep every answer between calls |
| Checking a PDF before sending it | A PDF that is ingested already costs nothing and says so, and `list_documents` shows what is stored |
| Saving a media before its first PDF | `ingest_pdf` makes a new media with its labels, and a media with no document holds nothing to read |
| Cancelling an ask | The client cancels its own call |
| Navigation and drawing: the shelf, the find box, panels, the concept graph, notices | An agent filters the list itself, and a tool says how it ended. The concepts that a search went through are in `search` with `explain` |

## 9. When a call fails

A failure comes back as a tool error (`isError: true`) with a text that says what to do. It never ends the connection.

| What went wrong | What the text says |
| --- | --- |
| A store or a service is down, or a key is missing | The error, the address or key it names, and: call the `health` tool |
| A bad argument: a blank question, an unknown `kind` or `category`, a blank tag, a bad `document_id`, a `limit` of 0, both `path` and `pdf_base64`, a `kind` of `chunk` for `chapter_contents` | What was given and what is allowed |
| `read_page` for a document that has no converted folder | The id, the content folder, and: call `list_documents` |
| `read_page` for a document that is not found, when some converted documents cannot be read | The id, the content folder, "the document may be one of the chapters that cannot be read" with the file of each, and: call `list_documents` |
| `read_page` for a page the document does not have | The page and how many pages there are |
| `job_status`, `watch_job` or `ingest_status` for an unknown `job_id` | Job ids are forgotten when the server restarts, and the server keeps only its last 100 jobs of each kind; for an ingest, send the PDF again |
| `watch_job` with a `timeout_s` that is not from 1 to 90 | That `timeout_s` must be from 1 to 90 seconds, and that leaving it out waits 60 |
| A PDF that is refused: no file at the path, not a file, not a PDF, too big, a `file_name` with a folder in it, a book's file name that is not `chapter-<number>-<name>.pdf` and has no `document_title` | The reason and the fix |
| A store that is down when the PDF is named, before the job starts | `could not connect to the graph`, or `could not read from the graph which media the library has`, with the address it names, and: call the `health` tool, and send the same PDF again |
| An ingest is already running | Its `job_id`, and: ask `ingest_status`, and send this PDF when that job has ended |
| `insights_status` or `stop_job` for an unknown `job_id` | Job ids are forgotten when the server restarts, and the server keeps only its last 100 jobs of each kind; for `insights_status`, call `write_insights` again, because insights that are saved are not asked for twice |
| `write_insights` without exactly one of `document_id`, `media` and `all: true` | That exactly one of `document_id`, `media` and `all: true` must be given. It is checked before any store is reached |
| `write_insights` for a document or media that is not stored | The id or title, and: call `list_documents` |
| `write_insights` when the agent of the Ingest choice is not signed in, for a job that is not a dry run | That no insights were written, to sign it in or give `dry_run: true`, and: call the `health` tool |
| An insights job is already running | Its `job_id`, and: ask `insights_status`, and call `write_insights` again when that job has ended |
| `chapter_insights` for a file that cannot be read | The file, and: call `write_insights` with the `document_id`, which is paid |
| `export_insights` with a `format` that is not `pdf`, `markdown` or `html`, a `path` that is not absolute, that does not end with the extension of the format, or whose folder is not there | What was given and what is allowed. Nothing is written |
| `export_insights` when a file is at `path` and `overwrite` is not `true` | The path, and: give another path, or `overwrite: true` to replace it |
| `export_insights` for a document with no saved insights | The id, and: call `write_insights` with this `document_id`, which is paid, then export again |
| `export_insights` for a part of a book that is not a chapter | The id, and that a part of a book gets no insights, so there is nothing to export |
| `export_insights` when the file cannot be written | That it could not be written, with the path and the reason that the system gave |
| `chapter_concepts` for a document that the graph does not hold | The id, and: call `list_documents` |
| `edit_media` or `edit_document_tags` with nothing to change, a blank tag, a bad `category` or a blank `media` | What is missing or not valid. It is checked before any store is reached, so nothing changes |
| `edit_media` for a media that is not stored, or `edit_document_tags` for a document that is not | The title or id, and: call `list_documents` |
| A paid call that fails because the credit of a provider is used up | The error, then `Your Google Gemini prepaid credit is used up: add credit at https://aistudio.google.com/billing, then try again.` or `Your Jev credit is used up: add credit in the console at https://console.typesafe.ai/, then try again.` |
| `dataset_status` for an unknown `job_id` | Job ids are forgotten when the server restarts, and the server keeps only its last 100 builds; call `build_dataset` again |
| A bad `name` for a dataset, or a blank one | What the name may be made of |
| A `media`, `documents` or `holdout` entry that no converted document matches, or no document left to build from | The entry, and: give a `document_id` or the whole `title` from `list_documents`, or the title of a media; for no document, check that the documents are converted to the end, and give `include_book_parts: true` for a part of a book |
| The model is not signed in, for a build that is not a dry run | That it is not signed in, to sign it in or give `dry_run: true`, and: call the `health` tool |
| The graph is down when a build starts | `could not connect to the graph` with the address it names, and: call the `health` tool |
| A dataset build is already running | Its `job_id`, and: ask `dataset_status`, and start this build when that job has ended |

## 10. Add it to Claude Code

### With `claude mcp add`

Connect a client on any machine to the server's one library:

```bash
claude mcp add --transport http quanty https://quanty.example.com/mcp --header "Authorization: Bearer YOUR_QUANTY_MCP_TOKEN"
```

Replace the host and token. Keep client configuration private; a bearer token grants access to the MCP tools, including paid jobs and server-side file exports. Add `--scope user` to register it for every project.

For a stdio client running on the server itself, build and install `quanty-mcp`, then use the service account and working directory:

```bash
claude mcp add quanty -- sh -c "cd '/srv/quanty' && exec '/usr/local/bin/quanty-mcp'"
```

### With `.mcp.json`

Place this private file in the client project's folder:

```json
{
  "mcpServers": {
    "quanty": {
      "type": "http",
      "url": "https://quanty.example.com/mcp",
      "headers": {
        "Authorization": "Bearer YOUR_QUANTY_MCP_TOKEN"
      }
    }
  }
}
```

For stdio on the host, replace that entry with:

```json
{
  "mcpServers": {
    "quanty": {
      "command": "sh",
      "args": ["-c", "cd '/srv/quanty' && exec '/usr/local/bin/quanty-mcp'"]
    }
  }
}
```

Another MCP client uses the same HTTPS URL and authorization header, or starts the same stdio command locally on the host.

Required artifact descriptions are part of PDF ingestion, including metadata-only repair when both stores already hold the document. `job_status` reports the stage `writing_artifacts` with `artifacts_done` and `artifacts_total`, and reports cumulative usage after accepted or rejected replies. Metadata failure ends the job as failed while preserving prior searchable data and accepted checkpoints. A missing converted folder or oversized pending request blocks repair. Source, embeddings and concept inputs keep their original content. Explicit bulk backfill is available through `rag-ingest artifacts --all --dry-run` and `rag-ingest artifacts --all`.

Picture references use `content/...` URLs, with escaped path segments. Resolve them against the registered app base (or the Jet mount for browser display); they never require access to the server filesystem. Paths outside the configured content folder are omitted.

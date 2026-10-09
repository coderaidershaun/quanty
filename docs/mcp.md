# The MCP server

`quanty-mcp` lets an AI agent use quanty: search the stored library of books, papers and other media, read the pages of its documents, and send a PDF to be ingested. It speaks the [Model Context Protocol](https://modelcontextprotocol.io), over standard input and output (the default) or over HTTP on this machine.

## 1. Before you start it

The server uses the same things as the other commands. Do the set up in [README.md](../README.md) first:

- Qdrant and FalkorDB are running (`docker compose up -d`).
- `.env` holds the two keys, `EMBEDDING_GEMINI_API_KEY` and `CONVERTER_JEV_API_KEY`.
- `claude` is signed in, and `ANTHROPIC_API_KEY` is not set.
- Poppler is installed (only `ingest_pdf` needs it).

The `health` tool tells an agent whether Qdrant, FalkorDB and the `claude` sign-in are ready. It does not check the two keys, `ANTHROPIC_API_KEY` or Poppler.

### Where it must start

The settings in `.env`, and the folders `content` and `data/…`, are found from the folder the command is started in. An agent starts a stdio server in the agent's own project folder, where there is no `.env`. So the server must start in the quanty folder, or every setting of `.env` (the two keys too) must be given as environment, with absolute folders. The snippets below start it in the quanty folder.

## 2. Start it

```bash
cargo build --release -p mcp          # makes target/release/quanty-mcp
```

| Transport | Command | Where it listens |
| --- | --- | --- |
| Standard input and output | `target/release/quanty-mcp` | the agent starts it and talks to it |
| HTTP | `target/release/quanty-mcp --http 8321` | `http://127.0.0.1:8321/mcp` |

The HTTP server listens on `127.0.0.1` only, and it has **no login**: any program on this machine can call it. It accepts only requests whose `Host` is this machine, and it grants no cross-site access, so a web page cannot call it. In stdio mode nothing but the protocol is written to standard output; the log goes to standard error.

## 3. The tools

Seven tools. A tool that costs money says so in its description, and so does this table.

| Tool | What it does | Costs |
| --- | --- | --- |
| `search` | The stored items (text chunks, formulas, figures, tables) nearest to a question | A small Gemini call |
| `answer` | An answer written from what `search` finds, with its sources | A Gemini call and your Claude usage; a minute or two |
| `list_documents` | Every stored document with its media, category, authors, tags and chapter | Nothing |
| `read_page` | The pieces of one page of a document, in reading order | Nothing, and it needs no store |
| `health` | Whether Qdrant, FalkorDB and the `claude` sign-in are ready | Nothing |
| `ingest_pdf` | Converts one PDF of a media and stores it | **Paid and slow**: Claude, Jev and Gemini; minutes for each PDF |
| `ingest_status` | How an ingest job is going | Nothing |

An optional argument can be left out or set to `null`. An optional text that is blank counts as left out.

First calls to try, in this order:

1. `health` with no arguments: `healthy` must be `true`.
2. `list_documents` with no arguments: the documents that are stored, each with its `document_id`.
3. `search` with `{"question": "What is the Black–Scholes formula for a call option?"}`: then give the `document_id` and `page` of a result to `read_page`.

### `search`

| Argument | |
| --- | --- |
| `question` | Required. The question, in plain words |
| `kind` | `chunk`, `formula`, `figure` or `table` |
| `media` | Only documents of this media, whatever the capitals. The older name `book` is understood too |
| `author` | Only documents by this author, whatever the capitals. It matches any author of a document |
| `category` | Only documents of this category: `book`, `paper` or `other` |
| `tags` | Only documents that have every one of these tags, each as a tag of the media or as an own tag of the document |
| `limit` | Keep only the first `limit` results (from 1) |
| `explain` | `true` adds `trace`, which says how the search got to the results |

Gives `results`, best first. Each result has `number`, `document_id`, `document` (its title: the book and the chapter, or a title of its own), `media` (the title of its media; a picture that stands alone has none), `category` (left out when there is no media), `authors`, `media_tags` (the tags of the media), `tags` (the document's own tags), `page` (the place of the page in the document, from 1), `printed_page` (the number printed on the page: cite with it), `kind`, `label`, `score`, `reason`, `text` and, for a figure, `picture` (the path of its picture on this machine). `reason` is `{"why": "nearest"}`, `{"why": "concept", "concept": …}` or `{"why": "cited", "by": …, "label": …}`. With `explain`, `trace` has `documents_searched`, `seeds`, `question_concepts`, `seed_concepts`, `related_concepts`, `candidates`, `ranked`, `capped` and `kept`. It also gives `usage` and `cost_usd`, as below. Give the `document_id` and `page` of a result to `read_page` to read around it.

### `answer`

The arguments `question`, `kind`, `media`, `author`, `category` and `tags` of `search`. Gives `answered` (false when nothing was found, and then Claude is not asked, or when what was found does not answer the question), `title`, `claims` (each with `heading`, `text` and the `sources` it rests on, by number), `sources` (each item that a claim names, once: `number` and the fields of a search result without `score` and `reason`) and `follow_ups`. The sign-in of `claude` and `ANTHROPIC_API_KEY` are checked before the search, which costs nothing, so nothing is embedded when `claude` cannot answer. An agent that can write its own answer should call `search`.

`search` and `answer` also give what the call used. `usage` has one entry for each model: `model`, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens` and `estimated` (true for the embedding, whose count is worked out from the length of the text). `cost_usd` is about what the call cost in US dollars at the prices of the API, or `null` when a model has no price; the table and its date are under [Costs](reference.md#costs). `search` uses the embedding alone. `answer` adds the model that wrote it, and when nothing was found it gives what the search used.

```json
"usage": [{"model": "gemini-embedding-2", "input_tokens": 12, "output_tokens": 0, "cache_read_tokens": 0, "cache_write_tokens": 0, "estimated": true}],
"cost_usd": 0.0000024
```

### `list_documents`

No arguments. Gives `documents`, sorted by title: `document_id`, `title`, `media` and `category` (left out for a picture that stands alone, which belongs to no media), `authors`, `media_tags` (the tags of the media), `tags` (the document's own tags) and, when the converted document is under the content folder, `pages`, with `chapter_number` and `chapter_name` for a book's chapter.

It also gives `unreadable_chapters`: one line for each converted document under the content folder whose `chapter.json` cannot be read, with the file that is at fault. The list is always there, and it is empty when every one can be read. Such a document shows no `chapter_number`, `chapter_name` or `pages`, and `read_page` cannot read it.

### `read_page`

| Argument | |
| --- | --- |
| `document_id` | Required. From a search result or `list_documents` |
| `page` | Required. The `page` of a search result (from 1), not the printed number |

Gives `document_id`, `media` (the title of its media), `chapter_number` and `chapter_name` (for a book's chapter), `document_title` (for a document with a title of its own, such as a paper's), `page`, `pages`, `printed_page`, `page_image` (the path of the picture of the whole page) and `pieces`: for each, `number`, `kind` (`heading`, `text`, `formula`, `figure`, `table` or `footnote`), `section` (the headings it sits under, outermost first), `label`, `content` and, for a figure, `picture`.

### `health`

No arguments. Gives `healthy` and `report`, one line for each of Qdrant, FalkorDB and claude.

### `ingest_pdf` and `ingest_status`

See the next part.

## 4. Sending a PDF

`ingest_pdf` takes the media, the PDF and labels:

| Argument | |
| --- | --- |
| `media` | Required. The title of the media the PDF belongs to, such as "Option Volatility and Pricing". The older name `book` is understood too |
| `category` | `book` (the default), `paper` or `other`. Used only when the library does not have this media yet: a stored media keeps its own category, which says how the PDF is named |
| `authors` | The authors of the media, in order. Used only when the library does not have this media yet |
| `tags` | The tags of the media. Used only when the library does not have this media yet |
| `document_tags` | The tags of this PDF only. They are added to the tags that the document has |
| `document_title` | The title of the document, for a paper or another media. It defaults to the title of the media. Not for a book, whose chapter is named by its file name |

An ingest never changes a stored media: to change its labels, use `rag-ingest media` or the Library tab of the desktop app. The media is found by its title whatever its capitals and the space at its ends.

The PDF is sent one of two ways. Give exactly one of them.

| Way | Arguments | For |
| --- | --- | --- |
| By path | `path`: the absolute path of the file on the machine the server runs on | An agent on the same machine. This is the way to use |
| As base64 | `pdf_base64`: the bytes as standard base64, and `file_name` | A program client. An AI agent cannot write megabytes of base64 itself |

Rules:

- The media is a book when the library has it as a book, and a new media is a book when `category` is left out or is `book`. The PDF of a book is named `chapter-<number>-<name>.pdf`, such as `chapter-1-financial-contracts.pdf`: the chapter comes from the file name. A book's file with another name must be copied or renamed first, and a book takes no `document_title`.
- The PDF of a paper or another media can have any name.
- It must be an existing file that starts with `%PDF-` and is at most 50 MiB.
- An upload (`pdf_base64`) is saved under the content folder, in `_uploads/<media folder>/<file name>`, and never at a path that the caller chooses. A `file_name` with a folder in it is refused.
- The converted pages are saved under the content folder, in `<media folder>/chapter-<number>/` for a book's chapter and in `<media folder>/<document title folder>/` for any other document. The folders are made from the titles: lower case, with one dash for each run of characters that are not letters or digits.

The call does not wait for the whole ingest, which takes minutes. It answers once the paid work has begun, with a `job_id` and `state: "running"`. Then the agent asks `ingest_status` with that `job_id` every 20 to 30 seconds until `state` is not `running`. Both tools give the same report: `job_id`, `state`, `media` (the title of the media), `file` (the file name) and, by `state`:

| `state` | Meaning |
| --- | --- |
| `running` | Going on. `stage` is `converting` (the pages are read: the slow part) or `ingesting` (the converted document is embedded, stored and searched for concepts). With no `stage` the stores are being checked |
| `done` | Finished. `document_id`, `items`, `summary` (what `rag-ingest pdf` prints), and `usage` and `cost_usd` (what the whole ingest used, in the shape that `search` gives them) are there |
| `already_ingested` | Both stores already held this PDF, so nothing was converted or embedded, and it costs nothing. `document_id` and `items` are there. The `document_tags` that were sent are still written |
| `failed` | Stopped. `error` says why. Send the same PDF again to go on: converted pages and kept answers are not paid for twice |

Good to know:

- One ingest runs at a time in a server. A second `ingest_pdf` while one runs is refused, and the refusal names the running `job_id`. This holds inside one server only: do not send the same PDF through two servers, or beside `rag-ingest pdf`, at the same time.
- The graph is read before the PDF is named, because a stored media's category says how the PDF is named. So a store that is down shows before a file name that breaks the rule for a book, as a tool error whose text names the store, says to call `health`, and says to send the same PDF again.
- A store that is down, a missing Gemini key, and a PDF that is already ingested show in the answer of `ingest_pdf` itself. A missing Jev key, `ANTHROPIC_API_KEY` being set, Poppler not installed, and a file that starts with `%PDF-` but is not a PDF are found only after the paid work has begun. So they show in `ingest_status` as `failed`, or, when they are found at once, in the answer of `ingest_pdf` as a tool error with the same text.
- Jobs live in the memory of the server. Over stdio a running job stops when the client closes the server, and a restarted server forgets its job ids. The server keeps only its last 100 jobs: when it holds 100 and a new one starts, the oldest, which has ended, is forgotten. Send the PDF again: a PDF that is ingested is not ingested twice, and a stopped one goes on from where it ended.

## 5. When a call fails

A failure comes back as a tool error (`isError: true`) with a text that says what to do. It never ends the connection.

| What went wrong | What the text says |
| --- | --- |
| A store or a service is down, or a key is missing | The error, the address or key it names, and: call the `health` tool |
| A bad argument: a blank question, an unknown `kind` or `category`, a blank tag, a bad `document_id`, a `limit` of 0, both `path` and `pdf_base64` | What was given and what is allowed |
| `read_page` for a document that has no converted folder | The id, the content folder, and: call `list_documents` |
| `read_page` for a document that is not found, when some converted documents cannot be read | The id, the content folder, "the document may be one of the chapters that cannot be read" with the file of each, and: call `list_documents` |
| `read_page` for a page the document does not have | The page and how many pages there are |
| `ingest_status` for an unknown `job_id` | Job ids are forgotten when the server restarts, and the server keeps only its last 100 jobs; send the PDF again |
| A PDF that is refused: no file at the path, not a file, not a PDF, too big, a `file_name` with a folder in it, a book's file name that is not `chapter-<number>-<name>.pdf`, a `document_title` for a book | The reason and the fix |
| A store that is down when the PDF is named, before the job starts | `could not connect to the graph`, or `could not read from the graph which media the library has`, with the address it names, and: call the `health` tool, and send the same PDF again |
| An ingest is already running | Its `job_id`, and: ask `ingest_status`, and send this PDF when that job has ended |

## 6. Add it to Claude Code

Build it first (`cargo build --release -p mcp`), and replace `/path/to/quanty` with the quanty folder. The folder is in single quotes inside the `-c` text, so that a folder with a space in its name works.

### With `claude mcp add`

```bash
# standard input and output
claude mcp add quanty -- sh -c "cd '/path/to/quanty' && exec '/path/to/quanty/target/release/quanty-mcp'"

# HTTP: start `quanty-mcp --http 8321` yourself, then
claude mcp add --transport http quanty http://127.0.0.1:8321/mcp
```

Run in the quanty folder, this form of the first command needs no path typed, because `$PWD` writes the absolute path of the folder into the entry. `--scope user` adds the server for every project, not only the current one:

```bash
claude mcp add --scope user quanty -- sh -c "cd '$PWD' && exec '$PWD/target/release/quanty-mcp'"
```

### With `.mcp.json`

Put this file in the project folder of the agent (or give the same entry to `claude mcp add-json`).

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

For HTTP:

```json
{
  "mcpServers": {
    "quanty": {
      "type": "http",
      "url": "http://127.0.0.1:8321/mcp"
    }
  }
}
```

Another agent that speaks MCP takes the same command, or the same URL.

# ingest_pdf

Converts one PDF of a book, a paper or another media and stores it so that it can be searched. Call `check_pdf` first: it is free and says the pages, the state and a cost estimate. It is PAID and slow: each page is read by the agent of the Ingest choice and Jev, the items are embedded by Gemini, and that agent writes the document's insights, takeaways and cheat sheet and finds the concepts, which takes minutes for a chapter.

The call answers when the paid work has started, with a `job_id`: call `watch_job` with it (it waits for you) or `job_status`. A PDF that is already ingested costs nothing, unless its insights are not saved yet: then the agent writes those alone. The stages are those of the progress bar of the browser app: `checking_stored`, `opening_pdf`, `preparing_pages`, `converting` (the slow part), `writing_graph`, `embedding`, `storing`, `writing_insights`, `writing_artifacts`, `reading_concepts`, `linking_concepts`.

The PDF is sent one of three ways; give exactly one: `path` (a file on the server's machine: the way to use), `pdf_base64` with `file_name` (a program client; a model cannot write megabytes of base64) or `pdf_url` with `file_name` (a PDF on another machine; the server fetches it with one GET, redirects are not followed, and only the server's own machine and the origins in `QUANTY_FETCH_ORIGINS` are allowed). It must start with `%PDF-` and be at most 50 MiB. An upload or fetched PDF is saved under the content folder in `_uploads/<media folder>/<file name>`.

A book's chapter must be named `chapter-<number>-<name>.pdf`; a part of a book that is not a chapter, such as its index, takes `document_title` and can have any file name; a paper or other PDF can have any name. The media is a book when the library has it as a book, and a new media is a book when `category` is left out. An ingest never changes a stored media: use `edit_media` for that.

`state` is `running`, `done` (with `document_id`, `items`, `summary`, `usage`, `cost_usd`), `already_ingested`, `failed` or `stopped`. After a failure or a stop, send the same PDF again to go on from where it ended: converted pages and kept answers are not paid for twice.

Unified HTTP permits only one background ingest, insights job or dataset build across the app and MCP. Stdio permits one job of each kind at a time. Do not send the same PDF through a second server, or beside `rag-ingest pdf`, at the same time. Jobs live in the memory of the server; a restarted server forgets them.

## Arguments

- `media`: required. The title of the media the PDF belongs to. The older name `book` is understood too.
- `category`: `book` (the default), `paper` or `other`. Used only when the library does not have this media yet: a stored media keeps its own category, which says how the PDF is named.
- `authors`: the authors of the media. Used only when the library does not have this media yet.
- `tags`: tags of the media, such as "options". Used only when the media is new.
- `document_tags`: tags of this PDF only.
- `document_title`: the title of this document. For a paper or other media it defaults to the title of the media. For a book it names a part that is not a chapter, such as "Index": the document is stored as "<book>, <title>". Leave it out for a book's chapter.
- `path`: the absolute path of the PDF on the machine the server runs on.
- `pdf_base64`: the bytes of the PDF as standard base64. It goes with `file_name`.
- `pdf_url`: an http or https URL the server fetches the PDF from. It goes with `file_name`.
- `file_name`: the name to save the bytes of `pdf_base64` or `pdf_url` under, with no folder in it. Not for `path`.

## Examples

```json
{ "media": "Option Volatility and Pricing", "path": "/srv/pdfs/chapter-1-introduction.pdf" }
```

```json
{ "media": "Some Paper", "category": "paper", "pdf_url": "http://127.0.0.1:8080/files/paper.pdf", "file_name": "paper.pdf" }
```

## Errors

- A PDF that is refused (no file, not a PDF, too big, a `file_name` with a folder in it, a book file not named `chapter-<number>-<name>.pdf` and no `document_title`) is a tool error with the reason and the fix.
- A `pdf_url` that is refused (origin not allowed, a redirect, a status that is not a success, a body over the limit or without `%PDF-`) says to send the PDF by `path`, or to ask whoever runs the server to add the origin to `QUANTY_FETCH_ORIGINS`.
- A store that is down is a tool error that says to call `health` and to send the same PDF again.
- Another job is running: the error names its `job_id`; ask `job_status` with that job_id and send this PDF when it has ended.
- A missing Jev key, Poppler not installed or a file that is not a PDF are found after the paid work has begun and show in `job_status` as `failed`.

## Cost

PAID and slow: the agent of the Ingest choice, Jev and Gemini; minutes for each chapter. `check_pdf` gives an estimate first. A PDF already ingested with its insights saved costs nothing.

## Related

- `check_pdf`
- `watch_job`
- `job_status`
- `stop_job`
- `edit_media`

# check_pdf

What an `ingest_pdf` of a PDF would do, before anything is paid. Give the same PDF arguments as `ingest_pdf`: `media`, `category`, `document_title`, and `path`, or `pdf_url` with `file_name`. It answers what the free check of the browser app answers when a PDF is chosen, and adds a rough estimate. It starts no job, holds no background slot and writes nothing to the stores or the content folder. A `pdf_url` is fetched into a temporary file that is removed when the check ends, and never saved under `_uploads`.

Fields: `media`, `category`, `file`; `title` (the title the document has once ingested) and `document_id`; `pages` (null when they cannot be counted, a blocker says why, and for a document that is stored already); `state`; `pages_done`, `items`, `missing_artifacts`, `missing_insights` (only for the states that name them); `blockers` (every reason an ingest would fail, one line each; empty means it may start) and `estimate` (`pages_to_convert`, `usd_low`, `usd_high`, `minutes`).

`state` is `new`, `partly_converted` (a start goes on from `pages_done`), `converted`, `ingested` (a start finds nothing to do), `ingested_without_insights` (a start asks the agent for them), `ingested_without_metadata`, or null when the folder of the document holds a different PDF.

The estimate is a range: `usd_low` if every page is a plain page and `usd_high` if every page is read as math or figures, each rounded up to the cent, and null when the price table has no price for a model. `minutes` counts about 28 seconds a page. Artifact descriptions, concepts, embedding and insights cost extra and are not in it.

## Arguments

- `media`: the title of the media the PDF belongs to, such as "Option Volatility and Pricing".
- `category`: `book` (the default), `paper` or `other`. Used only when the library does not have this media yet: a stored media keeps its own category.
- `document_title`: the title of this document, as `ingest_pdf` takes it.
- `path`: the absolute path of the PDF on the machine the server runs on.
- `pdf_url`: an http or https URL that the server fetches the PDF from, as `ingest_pdf` takes it. It goes with `file_name`.
- `file_name`: the name of the PDF fetched from `pdf_url`. Not for `path`.

## Examples

```json
{ "media": "Option Volatility and Pricing", "path": "/srv/pdfs/chapter-1-intro.pdf" }
```

## Errors

- What `ingest_pdf` would refuse is refused the same way, as a tool error: a blank media, no PDF or two, a file that is not a PDF, a `pdf_url` that is not allowed.

## Cost

Free. It reads the PDF header and the stores.

## Related

- `ingest_pdf`
- `list_documents`

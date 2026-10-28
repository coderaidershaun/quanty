# list_documents

Every stored document with its media, category, authors, tags and chapter, sorted by title. A document is one chapter of a book, a part of a book that is not a chapter, a paper or another PDF. Give the `title` or the `document_id` of one as `document` to `search` or `answer` to ask that document alone, and its `document_id` to the tools that read it.

`detail: full` (default) also gives `pages`, `chapter_number`, `chapter_name`, `ingested_whole` and `items` (how many items of each kind the stores hold: `chunk`, `formula`, `figure`, `table`, `code`). `summary` gives `document_id`, `title`, `media`, `category`, `pages` and `ingested_whole`. `names` gives `document_id` and `title` only, and the result lists nodes that a host can open (`document_outline`). `ingested_whole` is false when an ingest stopped or skipped an item, or when the document was stored before quanty kept this mark: a search may miss part of it, and sending its PDF again with `ingest_pdf` finishes it.

The result also has `total` (how many documents fit the filters), `cursor` (only when more fit than this page holds) and `unreadable_chapters`: one line for each converted document whose `chapter.json` cannot be read, with the file that is at fault. Such a document shows no `chapter_number`, `chapter_name` or `pages`, and `read_page` cannot read it.

## Arguments

- `media`: only documents of this media, whatever the capitals.
- `category`: only documents of this category: `book`, `paper` or `other`.
- `author`: only documents by this author, whatever the capitals.
- `tags`: only documents that have every one of these tags, each as a tag of the media or of the document.
- `detail`: `full` (default), `summary` or `names`.
- `cursor`: from a result that has one, with the same filters.
- `limit`: how many documents at most, from 1 to 500. 100 when left out.

## Examples

```json
{ "detail": "names" }
```

```json
{ "media": "Option Volatility and Pricing", "detail": "summary", "limit": 20 }
```

## Errors

- A graph that is down is a tool error that says to call `health`.
- A `limit` of 0 or over 500, a bad `category`, a blank tag or a `cursor` of another read is refused with what is allowed.

## Cost

Free. It reads FalkorDB and the content folder.

## Related

- `document_overview`
- `document_outline`
- `list_media`

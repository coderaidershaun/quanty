# write_insights

Writes the missing insights, takeaways and cheat sheets of documents that are already stored, from their converted text, with nothing embedded. It is PAID unless `dry_run` is true: the agent of the Ingest choice is asked once for each document that has none, which spends its subscription usage and takes a minute or more for each. A dry run lists what would be written, asks nothing and costs nothing: run one first.

Give exactly one of `document_id`, `media` and `all: true`. A document whose insights are saved is never asked about again, and a part of a book that is not a chapter gets none. The call answers once the work has begun, with a `job_id`: call `watch_job` with it (it waits for you) or `job_status`. The one stage is `writing_insights`, with `done` and `total` the documents handled and chosen.

The result has the fields of a job and `dry_run`, `done`, `total`, `current` (the title of the document being asked about) and `documents`, one line for each document handled with `document_id`, `title` and `outcome`: `would_write` (dry run), `written`, `already_saved`, `not_written` (with a `reason`; calling again tries once more), `none_for_book_part` or `no_converted_chapter`.

Unified HTTP permits only one background job across the app and MCP. Stdio permits one job of each kind at a time. `stop_job` keeps what was written: call again and saved insights are not asked for twice.

## Arguments

- `document_id`: one stored document: its `document_id` from `list_documents`.
- `media`: every document of this media, by title whatever the capitals.
- `all`: true for every stored document that belongs to a media.
- `dry_run`: list what would be written and ask nothing. It costs nothing. False when left out.

## Examples

```json
{ "document_id": "…", "dry_run": true }
```

## Errors

- Not exactly one of `document_id`, `media` and `all: true` is refused before any store is reached.
- A document or media that is not stored is a tool error that says to call `list_documents`.
- An agent that is not signed in, for a job that is not a dry run, fails the call: sign it in or give `dry_run: true`, and call `health`.
- Another background job is running: ask `job_status` with its job_id and call again when it has ended.

## Cost

PAID unless `dry_run` is true: the subscription usage of the agent of the Ingest choice, a minute or more for each document. A dry run is free.

## Related

- `chapter_insights`
- `watch_job`
- `job_status`
- `export_insights`

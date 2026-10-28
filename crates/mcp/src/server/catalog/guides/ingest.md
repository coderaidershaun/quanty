# Ingest

`ingest_pdf` adds one PDF to the library. It is PAID and slow: minutes for a chapter.

## Steps

1. `check_pdf` with the same arguments: the title it would be stored under, the pages, its state, anything that would stop it, and a rough cost range and time. It is free and writes nothing.
2. Fix every `blockers` line. Tell the person the cost and time if they have not agreed to them.
3. `ingest_pdf`. It answers at once with a `job_id` once the paid work has begun.
4. `watch_job` with the `job_id`: it waits until the stage or state changes (up to 90 seconds) and sends progress. Repeat until `state` is not `running`. `job_status` answers at once.

## Sending the PDF

Give exactly one of `path` (a file on the server's machine), `pdf_url` with `file_name` (the server fetches it; only its own machine and the origins in `QUANTY_FETCH_ORIGINS`; no redirects) or `pdf_base64` with `file_name` (for programs, not models). It must start with `%PDF-` and be at most 50 MiB.

## Names

A book's chapter is named `chapter-<number>-<name>.pdf`. A part of a book that is not a chapter (index, glossary) takes a `document_title` and any file name. A paper or other PDF can have any name. A stored media keeps its category; change labels with `edit_media`.

## After

`state: done` gives `document_id`, `items`, `summary`, `usage` and `cost_usd`. `already_ingested` means nothing was paid. After `failed` or `stopped`, send the same PDF again: converted pages and kept answers are not paid for twice. `write_insights` writes missing insights of stored documents (PAID unless `dry_run`).

## One job at a time

Unified HTTP allows one background ingest, insights job or dataset build across the app and MCP. A second start is refused with the running `job_id`.

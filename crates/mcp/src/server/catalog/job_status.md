# job_status

The state of a job of any kind by its `job_id`: an ingest, an insights job or a dataset build. It gives the fields every job has and, beside them, the fields of its kind.

- `job_id`, `kind` (`ingest`, `insights` or `dataset`), `state` (`running`, `done`, `already_ingested` for an ingest only, `failed` or `stopped`).
- `stage`: what a running job is doing, as `{ name, words, done, total, share }`. `name` does not change between versions, `words` is for a person, `done` and `total` are whole numbers and `share` is how much of the stage is done from 0 to 1; each is null for a stage that does not count. Left out when the job is not running and before its first step.
- `started_at`, `updated_at` (RFC 3339, UTC), `usage` and `cost_usd` (what the models used), and `error` (only when `failed` or `stopped`: why, and what to do).

An unknown `job_id` is a tool error: job ids are forgotten when the server restarts, and the server keeps only its last 100 jobs of each kind. The older `ingest_status`, `insights_status` and `dataset_status` give the same result for a job of their kind.

## Arguments

- `job_id`: the `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave.

## Examples

```json
{ "job_id": "…" }
```

## Errors

- A `job_id` this server does not hold: for an ingest, send the PDF again; for insights or a dataset, start the work again. Work already paid for is kept.

## Cost

Free.

## Related

- `watch_job`
- `list_jobs`
- `stop_job`

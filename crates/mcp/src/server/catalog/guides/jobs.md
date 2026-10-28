# Jobs

`ingest_pdf`, `write_insights` and `build_dataset` start a job and answer at once with a `job_id`. Every tool about a job gives the same view of it.

## The view

- `job_id`, `kind` (`ingest`, `insights`, `dataset`), `state`: `running`, `done`, `already_ingested` (ingest only), `failed`, `stopped`.
- `stage`: `{ name, words, done, total, share }` while running. `share` is 0 to 1; `done`, `total` and `share` are null for a stage that does not count.
- `started_at`, `updated_at`, `usage`, `cost_usd`, and `error` (why it stopped and what to do).
- The fields of the kind: `media`, `file`, `document_id`, `items`, `summary` for an ingest; `documents`, `done`, `total` for insights; `folder` and `summary` for a dataset.

## Follow

- `watch_job({ job_id })`: waits up to `timeout_s` (1 to 90, default 60) for a change of state or stage, then returns the view. With a progress token it sends progress notifications. This is the call to repeat; do not poll by hand.
- `job_status({ job_id })`: the view now.
- `list_jobs`: recent jobs of every kind and the job that holds the shared slot.
- A client that declares tasks gets a task instead of the report; `tasks/get` and `tasks/cancel` map to the job.

## Stop

`stop_job({ job_id })` stops a running job. What was paid for is kept; start the same work again to go on. Over stdio a job stops when the client closes the server.

## Forgotten ids

Jobs live in the memory of the server, the last 100 of each kind. After a restart a `job_id` is unknown: send the PDF again, or start the work again.

`ingest_status`, `insights_status` and `dataset_status` still answer but are deprecated: call `job_status`.

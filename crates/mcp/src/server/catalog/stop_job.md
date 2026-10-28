# stop_job

Stops a running job of this server, an ingest, an insights job or a dataset build, by the `job_id` that started it. A build can take hours, and a stopped job stops spending. What was paid for is kept, so the same work started again goes on from there: converted pages and kept answers of an ingest, the answers a model already gave in a dataset build, and the insights that are saved. A job that has ended already is left as it is.

Gives `job_id`, `kind`, `state` (`stopped` when the job was running and is stopped now, `ended` when it had ended already) and `status_tool`, which is `job_status`. The job then reports `state: "stopped"` with an `error` that says what to call again. A new job can start after the stopped work has ended.

## Arguments

- `job_id`: the `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave.

## Examples

```json
{ "job_id": "…" }
```

## Errors

- A `job_id` this server does not hold: job ids are forgotten when the server restarts, and the server keeps only its last 100 jobs of each kind.

## Cost

Free.

## Related

- `job_status`
- `list_jobs`

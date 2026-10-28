# ingest_status

Deprecated: call `job_status`, which gives the same result for a job of any kind. This tool says how an ingest job is going, or how it ended, and it will go once the programs that call it have moved. Over stdio a job stops when the client closes the server.

## Arguments

- `job_id`: the `job_id` that `ingest_pdf` gave.

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
- `ingest_pdf`

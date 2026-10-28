# watch_job

Waits until the stage or the state of a job changes, then gives the same result as `job_status`. It returns at once when the job is not running, on the first change of `state` or of the name of the `stage` since the call began, or at the timeout with the job as it is. A change of the counts or of the words inside one stage does not end the wait.

When the call carries a progress token, it also sends progress notifications while it waits: one at once, then at most one every 2 seconds, only when something changed. `progress` is the `share` of the stage in percent, `total` is 100 and `message` is the `words` of the stage. A client that cancels the call ends the wait and nothing else: the job goes on.

## Arguments

- `job_id`: the `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave.
- `timeout_s`: how long to wait for the stage or the state of the job to change, in seconds, from 1 to 90. 60 when left out.

## Examples

```json
{ "job_id": "…", "timeout_s": 60 }
```

## Errors

- A `timeout_s` that is not from 1 to 90 is refused; leaving it out waits 60.
- A `job_id` this server does not hold is a tool error.

## Cost

Free.

## Related

- `job_status`
- `stop_job`
- `ingest_pdf`

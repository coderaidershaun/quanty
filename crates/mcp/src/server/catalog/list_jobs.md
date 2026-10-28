# list_jobs

The jobs of this server of every kind, newest first, at most the last 20 of each kind, each as the fields of `job_status` without the fields of its kind. `running` is the work that holds the shared background slot of the unified HTTP server, as `{ kind, name }`, whether this server or the browser started it, and null when the slot is free (always null over stdio, which has no shared slot).

## Arguments

None.

## Examples

```json
{}
```

## Errors

- None: an empty list is a valid answer.

## Cost

Free.

## Related

- `job_status`
- `watch_job`

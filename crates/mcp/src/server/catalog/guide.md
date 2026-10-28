# guide

A short guide in Markdown (at most 6 KiB) on one topic, for an agent that has not used quanty before.

- `start`: what quanty is and the path to follow first.
- `research`: how to read the library and walk its concepts within a budget.
- `ingest`: how to send a PDF, check it first and follow the job.
- `jobs`: how to watch, list and stop jobs.
- `costs`: which tools are paid, who pays and how to avoid paying twice.
- `errors`: what a failed call says and what to do.

## Arguments

- `topic`: `start`, `research`, `ingest`, `jobs`, `costs` or `errors`.

## Examples

```json
{ "topic": "start" }
```

## Errors

- A topic that is not one of the six words is refused as a bad argument.

## Cost

Free. It reads text held by the server and costs nothing.

## Related

- `catalog`
- `describe_tool`

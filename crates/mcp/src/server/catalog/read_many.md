# read_many

Up to 20 reads in one call that share one `max_chars` budget. Each request is `{"tool": …, "args": {…}}` with a tool of `read_section`, `read_pages`, `read_item` or `chapter_insights`. The results come in order, each with `tool`, `truncated` and `result` or `error`, and `used_chars`.

Each request gets at most what is left of the budget (or its own smaller `max_chars`). A request with nothing left is not read and comes back with `truncated: true` and no result. A request that fails has an `error` and does not fail the call.

## Arguments

- `requests`: from 1 to 20 reads, done in order, each `{"tool": …, "args": {…}}`.
- `max_chars`: at most this many characters of text over all the reads. 24000 when left out.

## Examples

```json
{
  "requests": [
    { "tool": "read_section", "args": { "document_id": "…", "section": 1 } },
    { "tool": "chapter_insights", "args": { "document_id": "…" } }
  ],
  "max_chars": 12000
}
```

## Errors

- No request, more than 20, or a tool that is not one of the four is refused.
- A request that fails has its error in its own result.

## Cost

Free. It reads the converted files and needs no store.

## Related

- `read_section`
- `read_pages`
- `read_item`
- `chapter_insights`

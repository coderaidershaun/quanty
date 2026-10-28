# chapter_concepts

The concepts that one document discusses, as the ingest found them, the most mentioned first, each with a one-line definition. Give `page` for the concepts of one page only, as the browser app shows beside a page. `document_concepts` is the same with `limit`, `with_counts` and concept ids. It reads FalkorDB and costs nothing.

The result has `document_id`, `title`, `page` (only when asked) and `concepts` (`name` and `definition`). A page with no concept gives an empty list. A long list continues with `max_chars` (24000 when left out) and `cursor`; a cut falls inside a definition.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `page`: one page only: the `page` of a search result or of `chapter_contents` (from 1), not the number printed on the page. The whole document when left out.
- `max_chars`: at most this many characters of names and definitions. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…" }
```

## Errors

- A document that the graph does not hold is a tool error that says to call `list_documents`.
- A graph that is down is a tool error that says to call `health`.

## Cost

Free. It reads FalkorDB.

## Related

- `document_concepts`
- `concept`
- `document_overview`

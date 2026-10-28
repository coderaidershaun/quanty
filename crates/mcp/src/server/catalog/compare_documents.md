# compare_documents

Compares 2 to 10 documents by their concepts. `shared` lists the concepts that at least two of them mention, with how many items of each document mention it. `only_in` lists, for each document, the concepts that only it mentions. `truncated` is true when a list was cut at `limit`.

## Arguments

- `document_ids`: from 2 to 10 different `document_id`s from `list_documents`.
- `limit`: how many concepts at most in each list, from 1 to 500. Default 50.

## Examples

```json
{ "document_ids": ["…", "…"] }
```

## Errors

- Fewer than 2 or more than 10 documents, a repeated id or a limit that is not allowed is refused.
- A document that the graph does not hold is a tool error that says to call `list_documents`.

## Cost

Free. It reads the graph only.

## Related

- `document_concepts`
- `document_overview`

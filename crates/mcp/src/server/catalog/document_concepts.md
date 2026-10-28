# document_concepts

The concepts of a document, or of one page of it, as `chapter_concepts` gives them, with their `concept_id` (give one to `concept`), a `limit` and optional mention counts: with `with_counts`, `mentions` is the number of items of the whole document that mention each concept.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `page`: one page only: the `page` of a search result (from 1). The whole document when left out.
- `limit`: how many concepts at most, from 1 to 500. All when left out.
- `with_counts`: add `mentions`.

## Examples

```json
{ "document_id": "…", "limit": 20, "with_counts": true }
```

## Errors

- A limit that is not allowed, or a page the document does not have, is refused.
- A document that the graph does not hold is a tool error that says to call `list_documents`.

## Cost

Free. It reads the graph only.

## Related

- `chapter_concepts`
- `concept`
- `compare_documents`

# concept_items

The items that mention a concept, a page at a time. Each row has `item_id`, `document_id`, `page`, `printed_page`, `kind` and `wording` (the name the item gave the concept). When more are left, `next_cursor` is there. Read an item with `read_item` (by `item_id`) or the pages around it with `read_pages`.

## Arguments

- `concept_id`: the `concept_id` that a result gave.
- `document`: keep only the items of this document: a `document_id` from `list_documents`.
- `kind`: keep only items of this kind: `chunk`, `formula`, `figure`, `table` or `code`.
- `limit`: how many items at most, from 1 to 100. Default 20.
- `cursor`: the `next_cursor` of the previous result, to read the next page.

## Examples

```json
{ "concept_id": "…", "kind": "formula", "limit": 10 }
```

## Errors

- A limit that is not allowed, a bad kind or a cursor of another read is refused.
- A concept that is not stored is a tool error that says to call `find_concepts`.

## Cost

Free. It reads the graph only.

## Related

- `read_item`
- `read_pages`
- `concept`

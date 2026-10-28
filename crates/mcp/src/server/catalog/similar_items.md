# similar_items

The stored items nearest to a stored item by its stored vector, nearest first, with `document`, `page`, `label` and `score`. It reads Qdrant only and embeds nothing, so it is free.

## Arguments

- `item_id`: the `item_id` that a result gave.
- `limit`: how many items at most, from 1 to 50. Default 10.
- `kind`: keep only items of this kind: `chunk`, `formula`, `figure`, `table` or `code`.

## Examples

```json
{ "item_id": "…", "limit": 5 }
```

## Errors

- An item that is not stored is a tool error that says to call `search`.
- A limit that is not allowed or a bad kind is refused.
- A store that is down is a tool error that says to call `health`.

## Cost

Free. It reads Qdrant only.

## Related

- `read_item`
- `search`

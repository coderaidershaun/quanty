# concept_neighbours

The relations that touch a concept, one or two steps out: `DERIVED_FROM`, `ASSUMES`, `GENERALISES`, `PART_OF` and `USED_FOR`. Gives `edges` (`from`, `to`, `kind` and `stated_by`, the item, document and page that stated it, or null when that item is no longer stored), the `concepts` they touch, and `truncated`.

## Arguments

- `concept_id`: the `concept_id` that a result gave.
- `kinds`: keep only these kinds of relation. All when left out.
- `direction`: `out` for relations that leave the concept, `in` for those that reach it, or `both`. Default `both`.
- `depth`: 1 for the concepts next to it, 2 to go one step further. Default 1.
- `limit`: how many relations at most, from 1 to 200. Default 50.

## Examples

```json
{ "concept_id": "…", "direction": "out", "depth": 2 }
```

## Errors

- A depth, limit or kind that is not allowed is refused, never changed quietly.
- A concept that is not stored is a tool error that says to call `find_concepts`.

## Cost

Free. It reads the graph only.

## Related

- `concept`
- `concept_path`
- `concept_items`

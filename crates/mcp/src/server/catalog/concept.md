# concept

One concept: its definition, aliases, mention count, the documents that mention it with their counts, and its first five items. The result also lists nodes that a host can open: one for `concept_items`, one for `concept_neighbours` and one for each concept next to it (at most 20). Give `concept_id` or `name`, not both. With neither, the nodes lead to the most mentioned concepts.

## Arguments

- `concept_id`: the `concept_id` that a result gave.
- `name`: an exact name or alias, whatever its capitals.

## Examples

```json
{ "name": "long straddle" }
```

## Errors

- Both `concept_id` and `name`, or a concept id that is not valid, is refused.
- A concept that is not stored is a tool error that says to call `find_concepts`.
- A graph that is down is a tool error that says to call `health`.

## Cost

Free. It reads the graph only.

## Related

- `find_concepts`
- `concept_items`
- `concept_neighbours`

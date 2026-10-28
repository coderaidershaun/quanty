# find_concepts

Finds concepts in the graph that an ingest builds. Give exactly one of `name` and `query`. `name` is an exact name or alias, whatever its capitals, and is free. `query` finds concepts by meaning and is PAID: it embeds the query with Gemini once. Each concept comes with its `concept_id`, `name`, `definition`, `aliases`, `mentions` (items that mention it), `documents` and, for a `query`, `score`. For a `query` the result also has `usage` and `cost_usd`.

## Arguments

- `name`: an exact name or alias, whatever its capitals. Free.
- `query`: what the concept is about, in plain words. PAID: one embedding call.
- `limit`: how many concepts at most, from 1 to 25. Default 10.

## Examples

```json
{ "name": "long straddle" }
```

```json
{ "query": "a bet that the price moves a lot", "limit": 5 }
```

## Errors

- Both `name` and `query`, or neither, is refused with what to give.
- A `limit` of 0 or over 25 is refused, never changed quietly.
- A graph or store that is down is a tool error that says to call `health`.

## Cost

`name` is free. `query` is PAID: one Gemini embedding, a fraction of a cent.

## Related

- `concept`
- `concept_neighbours`
- `concept_items`

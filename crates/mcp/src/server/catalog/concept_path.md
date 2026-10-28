# concept_path

The shortest chain of relations between two concepts, followed either way, as `edges` in order (each with the item that stated it) and the `concepts` on the chain. Both are empty when no chain is that short.

## Arguments

- `from`: a `concept_id` or a name.
- `to`: a `concept_id` or a name.
- `max_depth`: the most relations in the chain, from 1 to 4. Default 4.

## Examples

```json
{ "from": "long straddle", "to": "volatility" }
```

## Errors

- A depth that is not from 1 to 4 is refused.
- A concept that is not stored is a tool error that says to call `find_concepts`.

## Cost

Free. It reads the graph only.

## Related

- `concept_neighbours`
- `find_concepts`

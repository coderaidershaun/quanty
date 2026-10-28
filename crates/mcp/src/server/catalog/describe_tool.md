# describe_tool

One tool in detail: the long description with its rules and caveats, notes on each argument, example calls as JSON objects, what can fail and what to do, what it costs, and the tools to use next. The tool list holds only a short description of each tool, so call this before the first use of a tool you do not know, above all a PAID one.

Each example is an `arguments` object for a call of the tool. Placeholders such as `"…"` stand for an id that an earlier call gave.

## Arguments

- `name`: the tool name, as `catalog` lists it.

## Examples

```json
{ "name": "ingest_pdf" }
```

## Errors

- A name that no tool has is a tool error that says to call `catalog` for the names.

## Cost

Free. It reads notes held by the server and costs nothing.

## Related

- `catalog`
- `guide`

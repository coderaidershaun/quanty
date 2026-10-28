# catalog

The index of the tools. With no `group` it lists the eight groups (catalog, library, research, concepts, jobs, ingest, datasets, admin) and the tools of each, one line per tool, and a short `start_here` path. With a `group` it lists only that group's tools. A host that browses tools can open the `nodes`: a node of kind `discover` calls `catalog` for a group, a node of kind `tool` calls a tool with suggested arguments (a draft, never run by the host while browsing).

`summaries` gives each tool's `name`, its `summary` (the short description from the tool list) and `paid`. `names` gives the names only, which is the smallest answer. A tool that spends money has `paid: true` and PAID as the second word of its description.

Call `describe_tool` for one tool in detail and `guide` for a short guide to a topic.

## Arguments

- `group`: one of `catalog`, `library`, `research`, `concepts`, `jobs`, `ingest`, `datasets`, `admin`. Left out: every group.
- `detail`: `summaries` (default) or `names`.

## Examples

```json
{}
```

```json
{ "group": "library", "detail": "names" }
```

## Errors

- A `group` or `detail` that is not one of the listed words is refused as a bad argument; the schema lists the allowed words.

## Cost

Free. It reads the tool list and costs nothing.

## Related

- `describe_tool`
- `guide`
- `health`

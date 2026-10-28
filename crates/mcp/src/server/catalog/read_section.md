# read_section

One section of a document, as Markdown (`as: markdown`, default) or as its pieces (`as: pieces`), in reading order: formulas as `$…$`, figures as picture URLs with the explanation of the figure, tables as Markdown, code listings fenced. The result has the section's `index`, `path`, `pages` (the first and last page of the section), `item_ids` (give one to `read_item`), `pictures` and `text` or `pieces`. The header gives the page count of the whole document as `document_pages`.

A long section is cut at `max_chars` (24000 when left out, at most 200000, counted in characters of the main text). The result then has `truncated: true` and a `cursor`; give the `cursor` back with the same other arguments to read on from exactly where the text stopped, so the reads put together are the whole text. A cursor of another read is refused.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `section`: the `index` of a section of `document_outline`, or its heading, whatever the capitals.
- `as`: `markdown` (default) or `pieces`.
- `max_chars`: at most this many characters of text. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…", "section": 1 }
```

## Errors

- A section that the outline does not have, or a heading that matches none, is a tool error that says to call `document_outline`.
- A `max_chars` of 0 or over 200000, or a cursor of another read, is refused.

## Cost

Free. It reads the converted files and needs no store.

## Related

- `document_outline`
- `read_pages`
- `read_item`

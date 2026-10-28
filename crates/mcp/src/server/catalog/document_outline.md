# document_outline

The sections of a document, in reading order. Each section has its `index` (counted from 1: give it to `read_section`), `path` (the headings above its text, outermost first), `rank` (the level of its last heading, from 1 for a chapter title down to 4), `printed_number` (the number the book prints before the heading), `pages` (its first and last page), `chars` (the characters of its Markdown), `items` (how many formulas, figures, tables and code listings it holds) and `concepts` (how many different concepts its items mention).

A graph that is down leaves `concepts` out of every section and puts the reason in `concepts_error`. The result lists nodes that a host can open, one `read_section` call for each section.

## Arguments

- `document_id`: from `list_documents` or a search result.

## Examples

```json
{ "document_id": "…" }
```

## Errors

- A document id that is not valid, or a document with no converted folder, is a tool error that says to call `list_documents`.

## Cost

Free. It reads the converted files and, for the concept counts only, FalkorDB.

## Related

- `read_section`
- `document_overview`
- `read_pages`

# chapter_contents

Every formula, figure, table and code listing of one document (a book's chapter, a part of a book or a paper), in reading order, each with the page it is on, its section, label, caption or name, and its whole text: the LaTeX of a formula, the code as printed, the explanation of a figure with the app-relative URL of its picture, the Markdown of a table. A code listing that page breaks cut is one item: `content` holds all of it, and `last_page` and `last_printed_page` say where it ends.

It reads the saved chapter, needs no store and costs nothing. The result has the same header as `read_page`, then `counts` (`formula`, `figure`, `table` and `code`, always of the whole document, whatever `kind` asked) and `items`. Give the `page` of an item to `read_page` to read around it. A long list continues with `max_chars` and `cursor`.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `kind`: `formula`, `figure`, `table` or `code`. Every kind when left out. `chunk` is refused: read the text of a page with `read_page`.
- `max_chars`: at most this many characters of text. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…", "kind": "formula" }
```

## Errors

- A document with no converted folder is a tool error that says to call `list_documents`.
- A `kind` of `chunk` or an unknown kind is refused with the allowed words.

## Cost

Free. It reads the converted files and needs no store.

## Related

- `read_item`
- `read_page`
- `document_outline`

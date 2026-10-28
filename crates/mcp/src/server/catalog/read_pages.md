# read_pages

A run of pages of a document, from page `from` to page `to`, as Markdown (default) or as pieces. Pages are counted from 1 and are the place of the page in the document, not the number printed on it. A paragraph that starts on a page of the run is given whole, even when it ends on the next page.

A long run is cut at `max_chars` (24000 when left out, at most 200000). The result then has `truncated: true` and a `cursor` to give back with the same other arguments.

## Arguments

- `document_id`: from `list_documents` or a search result.
- `from`: the first page, counted from 1.
- `to`: the last page, no earlier than `from`.
- `as`: `markdown` (default) or `pieces`.
- `max_chars`: at most this many characters of text. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…", "from": 1, "to": 3 }
```

## Errors

- A page the document does not have, or a `to` before `from`, is a tool error that says how many pages there are.
- A `max_chars` of 0 or over 200000, or a cursor of another read, is refused.

## Cost

Free. It reads the converted files and needs no store.

## Related

- `read_section`
- `read_page`
- `read_item`

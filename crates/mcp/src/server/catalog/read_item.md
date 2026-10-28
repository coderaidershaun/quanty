# read_item

One formula, figure, table or code listing, by its printed label (with `kind` and `document_id`) or by its `item_id`. It gives the item with its `item_id`, `page`, `section`, `section_index`, `label`, `caption`, `name` (a formula's name), `language` (code), `content` (the LaTeX of a formula, the code as printed, the explanation of a figure, the Markdown of a table), `picture` (a figure) and `cited_by`: the paragraphs that cite its label.

A long content is cut at `max_chars` (24000 when left out) and continues with `cursor`.

## Arguments

- `document_id`: needed with `kind` and `label`. With `item_id` it is the document to look in, which is faster.
- `kind`: `formula`, `figure`, `table` or `code`. With `label`.
- `label`: the label as printed, such as "(7.3)" or "Figure 13-4". With `kind`.
- `item_id`: the `item_id` of a search result or of `read_section`. Instead of `kind` and `label`.
- `max_chars`: at most this many characters of content. 24000 when left out.
- `cursor`: from a result that was `truncated`, with the same other arguments.

## Examples

```json
{ "document_id": "…", "kind": "formula", "label": "(7.3)" }
```

```json
{ "item_id": "…" }
```

## Errors

- Both a label and an `item_id`, or neither, is refused with what to give.
- A label that the document does not have is a tool error that says to call `chapter_contents`.

## Cost

Free. It reads the converted files and needs no store.

## Related

- `chapter_contents`
- `read_section`
- `similar_items`
